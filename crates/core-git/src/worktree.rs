//! Linked worktrees: listing, adding and removing them.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, LazyLock, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::GitError;
use crate::git::Git;
use crate::worktree_locks::fill_lock_reasons;
use crate::worktree_porcelain::parse_worktree_list;
use crate::worktree_prune::fill_prune_reasons;

/// One worktree from `git worktree list --porcelain`. The first entry is the main worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeEntry {
    pub path: PathBuf,
    /// Commit at HEAD; absent for a bare repository.
    pub head: Option<String>,
    /// Short branch name such as `main`; absent when HEAD is detached.
    pub branch: Option<String>,
    pub bare: bool,
    pub detached: bool,
    /// Lock reason; `Some("")` when the worktree is locked without a reason. Git before
    /// 2.31 prints no `locked` line, so the reason comes from git's `locked` admin file.
    pub locked: Option<String>,
    /// Why git could prune this worktree; `Some("")` when no reason is given. Git before 2.31 never
    /// prints it, so the folder's absence fills it in (`worktree_prune`). Do not use it to find a
    /// missing workspace: check that its directory exists.
    pub prunable: Option<String>,
}

impl WorktreeEntry {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            head: None,
            branch: None,
            bare: false,
            detached: false,
            locked: None,
            prunable: None,
        }
    }
}

/// How `worktree_add` obtains the branch it checks out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeAdd<'a> {
    /// Creates `branch` starting at `base`, or at HEAD when `base` is `None`.
    NewBranch {
        branch: &'a str,
        base: Option<&'a str>,
    },
    /// Checks out a local branch that already exists.
    ExistingBranch { branch: &'a str },
}

/// State recorded before `worktree add`.
#[derive(Debug)]
struct AddSnapshot {
    registered: Vec<PathBuf>,
    /// Something was already at the target path, so an undo must not delete it.
    target_existed: bool,
    /// The commit a branch this add creates will start at; `None` when the add
    /// creates no branch or the branch already exists.
    new_branch_base: Option<String>,
}

/// How long an undo keeps retrying. On Windows a killed add's processes exit asynchronously, and until they
/// do, a hook whose working directory is the new worktree keeps git from removing it.
const UNDO_BUDGET: Duration = Duration::from_secs(30);
const UNDO_RETRY: Duration = Duration::from_millis(250);
/// Each git command of an undo; long enough for a loaded machine, short enough to stay within the budget.
const UNDO_STEP_TIMEOUT: Duration = Duration::from_secs(15);

/// `path` in a form two spellings of one directory share; unchanged when it does not exist.
fn comparable(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

impl Git {
    /// Lists the main worktree and every linked worktree of `repo`. Paths with a line break
    /// are unsupported: before Git 2.36 `worktree list --porcelain` has no `-z` form to
    /// report them, so `worktree_add` refuses to create one.
    pub fn worktree_list(&self, repo: &Path) -> Result<Vec<WorktreeEntry>, GitError> {
        let captured = self.run(Some(repo), ["worktree", "list", "--porcelain"])?;
        captured.require_success()?;
        let text = captured.text()?;
        let mut entries =
            parse_worktree_list(text).map_err(|reason| captured.invalid_output(reason))?;
        fill_lock_reasons(&mut entries);
        fill_prune_reasons(&mut entries);
        Ok(entries)
    }

    /// Adds a worktree at `path`; a relative `path` resolves against `repo`. A `path` with a
    /// line break is refused before git runs. When git fails, or is killed by a mutation
    /// timeout, after registering the worktree or creating the branch, this call removes
    /// what it created so a retry can succeed.
    pub fn worktree_add(
        &self,
        repo: &Path,
        path: &Path,
        add: WorktreeAdd<'_>,
    ) -> Result<(), GitError> {
        if path.to_string_lossy().contains(['\n', '\r']) {
            return Err(GitError::UnsupportedPath {
                path: path.to_path_buf(),
            });
        }
        // `--` ends option parsing, so a path or base that starts with a dash is not an option.
        let mut args = vec![OsString::from("worktree"), OsString::from("add")];
        match add {
            WorktreeAdd::NewBranch { branch, base } => {
                args.push(OsString::from("-b"));
                args.push(OsString::from(branch));
                args.push(OsString::from("--"));
                args.push(path.into());
                args.extend(base.map(OsString::from));
            }
            WorktreeAdd::ExistingBranch { branch } => {
                args.push(OsString::from("--"));
                args.push(path.into());
                args.push(OsString::from(branch));
            }
        }
        let target = repo.join(path);
        // Held across the snapshot, the add and its undo: a second add to the same target that took its snapshot
        // while the first was running would otherwise see an empty path, fail, and undo the first one's worktree.
        let _claim = TargetClaim::acquire(&target);
        let before = self.add_snapshot(repo, &target, add)?;
        let result = self
            .run_mutation(Some(repo), args)
            .and_then(|captured| captured.require_success());
        if result.is_err() {
            self.undo_add(repo, &target, add, &before);
        }
        result
    }

    /// What exists before `worktree add` runs, so a failure undoes only what it created.
    fn add_snapshot(
        &self,
        repo: &Path,
        target: &Path,
        add: WorktreeAdd<'_>,
    ) -> Result<AddSnapshot, GitError> {
        let registered = self.registered_paths(repo)?;
        let new_branch_base = match add {
            WorktreeAdd::NewBranch { branch, base } => {
                if self
                    .resolve(repo, &format!("refs/heads/{branch}"))?
                    .is_some()
                {
                    None
                } else {
                    self.resolve(repo, base.unwrap_or("HEAD"))?
                }
            }
            WorktreeAdd::ExistingBranch { .. } => None,
        };
        Ok(AddSnapshot {
            registered,
            target_existed: std::fs::symlink_metadata(target).is_ok(),
            new_branch_base,
        })
    }

    /// Best effort: removes a worktree at `target` that was not registered before, and the directory if this
    /// add created it, retrying for up to [`UNDO_BUDGET`] while dying processes still hold it; then deletes a
    /// new branch that still points at its base. The add's own error is what the caller sees.
    fn undo_add(&self, repo: &Path, target: &Path, add: WorktreeAdd<'_>, before: &AddSnapshot) {
        // The add's own mutation timeout may be what cut it short; on a loaded machine every cleanup command would
        // hit it too, so the undo bounds each command on its own.
        let undo = self.clone().with_mutation_timeout(Some(UNDO_STEP_TIMEOUT));
        let target = comparable(target);
        let deadline = Instant::now() + UNDO_BUDGET;
        loop {
            // Every step on every pass: a step that keeps failing must not keep the others from running.
            // The branch delete fails while the worktree is still registered, and succeeds on a later pass.
            let registration = undo.undo_registration(repo, &target, before);
            let directory = undo_directory(&target, before);
            let branch = undo.undo_branch(repo, add, before);
            if registration && directory && branch {
                return;
            }
            if Instant::now() >= deadline {
                let left: Vec<&str> = [
                    (!registration).then_some("worktree registration"),
                    (!directory).then_some("directory"),
                    (!branch).then_some("branch"),
                ]
                .into_iter()
                .flatten()
                .collect();
                tracing::warn!(
                    target = %target.display(),
                    ?left,
                    "a failed worktree add could not be fully undone"
                );
                return;
            }
            thread::sleep(UNDO_RETRY);
        }
    }

    /// Deletes the branch this add created while it still points at its base; true once nothing of it is left
    /// to delete. A branch that moved since is someone's work and is kept.
    fn undo_branch(&self, repo: &Path, add: WorktreeAdd<'_>, before: &AddSnapshot) -> bool {
        let (WorktreeAdd::NewBranch { branch, .. }, Some(base)) = (add, &before.new_branch_base)
        else {
            return true;
        };
        let reference = format!("refs/heads/{branch}");
        match self.resolve(repo, &reference) {
            Ok(Some(commit)) if commit == *base => {
                let _ = self.run_mutation(Some(repo), ["branch", "-D", "--", branch]);
                matches!(self.resolve(repo, &reference), Ok(None))
            }
            Ok(_) => true,
            Err(_) => false,
        }
    }

    /// Unregisters `target` when this add registered it; true once git no longer lists it.
    fn undo_registration(&self, repo: &Path, target: &Path, before: &AddSnapshot) -> bool {
        let Ok(after) = self.registered_paths(repo) else {
            return false;
        };
        if !after.iter().any(|path| path == target)
            || before.registered.iter().any(|path| path == target)
        {
            return true;
        }
        // Twice `--force`: git locks a worktree while it is being added.
        let _ = self.run_mutation(
            Some(repo),
            [
                OsString::from("worktree"),
                OsString::from("remove"),
                OsString::from("--force"),
                OsString::from("--force"),
                OsString::from("--"),
                target.into(),
            ],
        );
        let _ = self.run_mutation(Some(repo), ["worktree", "prune"]);
        self.registered_paths(repo)
            .is_ok_and(|after| !after.iter().any(|path| path == target))
    }

    /// Canonical paths of every registered worktree, under the mutation timeout
    /// because it is part of an add.
    fn registered_paths(&self, repo: &Path) -> Result<Vec<PathBuf>, GitError> {
        let captured = self.run_mutation(Some(repo), ["worktree", "list", "--porcelain"])?;
        captured.require_success()?;
        let entries = parse_worktree_list(captured.text()?)
            .map_err(|reason| captured.invalid_output(reason))?;
        Ok(entries
            .iter()
            .map(|entry| comparable(&entry.path))
            .collect())
    }

    /// The commit `rev` names, or `None` when it names none.
    fn resolve(&self, repo: &Path, rev: &str) -> Result<Option<String>, GitError> {
        if rev.starts_with('-') {
            return Ok(None);
        }
        let commit = format!("{rev}^{{commit}}");
        let captured = self.run_mutation(
            Some(repo),
            ["rev-parse", "--verify", "--quiet", commit.as_str()],
        )?;
        if !captured.succeeded() {
            return Ok(None);
        }
        Ok(Some(captured.text()?.trim().to_owned()))
    }

    /// Removes the worktree at `path`. A dirty worktree is refused unless `force`; a locked
    /// one is refused either way, because git would need `--force` twice to remove it.
    pub fn worktree_remove(&self, repo: &Path, path: &Path, force: bool) -> Result<(), GitError> {
        let mut args = vec![OsString::from("worktree"), OsString::from("remove")];
        if force {
            args.push(OsString::from("--force"));
        }
        args.push(OsString::from("--"));
        args.push(path.into());
        self.run_mutation(Some(repo), args)?.require_success()
    }
}

/// Deletes the directory a failed add created, once git has let go of it; true when none of it is left.
/// A directory that was there before the add is never touched.
fn undo_directory(target: &Path, before: &AddSnapshot) -> bool {
    if before.target_existed {
        return true;
    }
    match std::fs::symlink_metadata(target) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(_) => false,
        Ok(_) => std::fs::remove_dir_all(target).is_ok(),
    }
}

/// Targets of the adds running in this process.
static CLAIMED_TARGETS: LazyLock<(Mutex<HashSet<PathBuf>>, Condvar)> =
    LazyLock::new(|| (Mutex::new(HashSet::new()), Condvar::new()));

/// One add's exclusive claim on its target path within this process; released on drop. Another process (a user's
/// terminal) can still race an add, and the snapshot check is what limits the damage then.
#[derive(Debug)]
struct TargetClaim {
    key: PathBuf,
}

impl TargetClaim {
    fn acquire(target: &Path) -> Self {
        let key = claim_key(target);
        let (claimed, released) = &*CLAIMED_TARGETS;
        let mut claimed = claimed.lock().unwrap_or_else(PoisonError::into_inner);
        while claimed.contains(&key) {
            claimed = released
                .wait(claimed)
                .unwrap_or_else(PoisonError::into_inner);
        }
        claimed.insert(key.clone());
        Self { key }
    }
}

impl Drop for TargetClaim {
    fn drop(&mut self) {
        let (claimed, released) = &*CLAIMED_TARGETS;
        claimed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.key);
        released.notify_all();
    }
}

/// One key for every spelling of a target that may not exist yet: absolute, and case-folded on Windows.
fn claim_key(target: &Path) -> PathBuf {
    let absolute = std::path::absolute(target).unwrap_or_else(|_| target.to_path_buf());
    if cfg!(windows) {
        PathBuf::from(absolute.to_string_lossy().to_lowercase())
    } else {
        absolute
    }
}
