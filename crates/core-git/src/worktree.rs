//! Linked worktrees: listing, adding and removing them.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::error::GitError;
use crate::git::Git;
use crate::worktree_porcelain::parse_worktree_list;

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
    /// 2.31 prints no `locked` line, so this is always `None` there, even when locked.
    pub locked: Option<String>,
    /// Why git could prune this worktree; `Some("")` when no reason is given. Do not use it
    /// to find a missing workspace: check that its directory exists. Git before 2.31 never
    /// reports it.
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
    /// The commit a branch this add creates will start at; `None` when the add
    /// creates no branch or the branch already exists.
    new_branch_base: Option<String>,
}

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
        parse_worktree_list(text).map_err(|reason| captured.invalid_output(reason))
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
        let before = self.add_snapshot(repo, add)?;
        let result = self
            .run_mutation(Some(repo), args)
            .and_then(|captured| captured.require_success());
        if result.is_err() {
            self.undo_add(repo, &repo.join(path), add, &before);
        }
        result
    }

    /// What exists before `worktree add` runs, so a failure undoes only what it created.
    fn add_snapshot(&self, repo: &Path, add: WorktreeAdd<'_>) -> Result<AddSnapshot, GitError> {
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
            new_branch_base,
        })
    }

    /// Best effort: removes a worktree at `target` that was not registered before, then
    /// deletes a new branch that still points at its base. The add's own error is what
    /// the caller sees, so failures here are ignored.
    fn undo_add(&self, repo: &Path, target: &Path, add: WorktreeAdd<'_>, before: &AddSnapshot) {
        let target = comparable(target);
        if let Ok(after) = self.registered_paths(repo) {
            for path in after
                .iter()
                .filter(|path| **path == target && !before.registered.contains(path))
            {
                // Twice `--force`: git locks a worktree while it is being added.
                let _ = self.run_mutation(
                    Some(repo),
                    [
                        OsString::from("worktree"),
                        OsString::from("remove"),
                        OsString::from("--force"),
                        OsString::from("--force"),
                        OsString::from("--"),
                        path.into(),
                    ],
                );
            }
            let _ = self.run_mutation(Some(repo), ["worktree", "prune"]);
        }
        if let (WorktreeAdd::NewBranch { branch, .. }, Some(base)) = (add, &before.new_branch_base)
        {
            let reference = format!("refs/heads/{branch}");
            if self.resolve(repo, &reference).ok().flatten().as_ref() == Some(base) {
                let _ = self.run_mutation(Some(repo), ["branch", "-D", "--", branch]);
            }
        }
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
