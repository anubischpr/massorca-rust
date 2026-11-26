//! Removing workspaces (ADR 0008): refusals first, then git, then the row, then the branch.

use std::path::Path;

use massorca_core_store::{
    ProjectRow, WorkspaceKind as StoreWorkspaceKind, WorkspaceRow,
    WorkspaceState as StoreWorkspaceState,
};
use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceRemoved};

use massorca_core_git::GitError;

use crate::error::DomainError;
use crate::paths;
use crate::views;
use crate::workspaces::Workspaces;

/// What the caller waives when removing a workspace. Each waiver covers one kind of change that
/// removal would otherwise refuse, and none of them covers another.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RemovalWaivers {
    /// Discards uncommitted and untracked changes in a registered worktree (`--force`).
    pub force: bool,
    /// Deletes ignored files in a registered worktree (`--discard-ignored`).
    pub discard_ignored: bool,
    /// Forgets a row whose directory git does not recognise as its worktree (`--forget-orphan`).
    pub forget_orphan: bool,
}

/// How the checks found a row. Removal acts on this reading, so a later look at git cannot change
/// what the waivers covered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    /// A folder workspace, which has no git state to check.
    Folder,
    /// A worktree git recognises, whose changes the waivers cleared.
    Worktree,
    /// A directory git does not recognise, kept whole under `forget_orphan`.
    Orphan,
}

impl RemovalWaivers {
    /// No waiver: every kind of change refuses removal.
    pub const NONE: Self = Self {
        force: false,
        discard_ignored: false,
        forget_orphan: false,
    };
    /// Discards uncommitted and untracked changes, but leaves ignored files refusing removal.
    pub const FORCE: Self = Self {
        force: true,
        discard_ignored: false,
        forget_orphan: false,
    };
}

/// What removing a workspace would meet, read without changing anything (decision 0163).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovalPreview {
    /// The workspace as it is now.
    pub workspace: Workspace,
    /// The refusal a removal with the given waivers would meet; `None` when it would go ahead.
    pub refusal: Option<DomainError>,
    /// The worktree has uncommitted changes to tracked files.
    pub dirty: bool,
    /// The worktree has untracked files.
    pub untracked: bool,
    /// The worktree holds ignored files that removal would delete.
    pub ignored: bool,
}

impl Workspaces {
    /// The workspace `selector` names and the workspaces its delegated children run in, deepest first (decision
    /// 0110). Each child is removed through [`Workspaces::begin_remove`] before the workspace itself. When there are
    /// children, the workspace's own refusals under `waivers` come first, read-only, so a refused workspace loses none.
    pub fn delegated_child_workspaces(
        &self,
        selector: &str,
        waivers: RemovalWaivers,
    ) -> Result<(Workspace, Vec<Workspace>), DomainError> {
        let row = self.resolve_workspace(selector)?;
        let children = self.store.list_delegated_child_workspaces(&row.id)?;
        if !children.is_empty() {
            self.check_removable(&row, waivers)?;
        }
        Ok((
            views::workspace(&row),
            children.iter().map(views::workspace).collect(),
        ))
    }

    /// What removing `selector` with `waivers` would meet, without marking or changing anything: the same refusals
    /// [`Workspaces::begin_remove`] runs, and the worktree's changes. A folder workspace has no git state to report.
    pub fn removal_preview(
        &self,
        selector: &str,
        waivers: RemovalWaivers,
    ) -> Result<RemovalPreview, DomainError> {
        let _guard = self.lock_mutations();
        let row = self.resolve_workspace(selector)?;
        let refusal = self.check_removable(&row, waivers).err();
        let path = Path::new(&row.path);
        let (dirty, untracked, ignored) =
            if row.kind == StoreWorkspaceKind::Worktree && path.is_dir() {
                match self.git.status_changes(path) {
                    Ok(changes) => (
                        changes.uncommitted > 0,
                        changes.untracked > 0,
                        changes.ignored > 0,
                    ),
                    Err(_) => (false, false, false),
                }
            } else {
                (false, false, false)
            };
        Ok(RemovalPreview {
            workspace: views::workspace(&row),
            refusal,
            dirty,
            untracked,
            ignored,
        })
    }

    /// Runs the refusals of removing `selector` with `waivers` without marking or changing anything, so a caller can
    /// refuse before it changes something else (decision 0163). A later removal runs them again.
    pub fn check_remove(&self, selector: &str, waivers: RemovalWaivers) -> Result<(), DomainError> {
        let _guard = self.lock_mutations();
        let row = self.resolve_workspace(selector)?;
        self.check_removable(&row, waivers).map(drop)
    }

    /// [`Workspaces::begin_remove`] with `guard` run first under the same mutation lock, before the row is marked
    /// `removing` (decision 0163): a caller that must know nothing else uses the workspace checks that here, where no
    /// removal can interleave. With `expect_clean`, a worktree that now has uncommitted or untracked changes is refused
    /// with `workspace.dirty` whatever `waivers` say, since the person saw it clean. A refusal leaves the row as it was.
    pub fn begin_remove_guarded<E: From<DomainError>>(
        &self,
        selector: &str,
        waivers: RemovalWaivers,
        expect_clean: bool,
        guard: impl FnOnce(&Workspace) -> Result<(), E>,
    ) -> Result<Workspace, E> {
        let _guard = self.lock_mutations();
        let row = self.resolve_workspace(selector)?;
        guard(&views::workspace(&row))?;
        let path = Path::new(&row.path);
        if expect_clean && row.kind == StoreWorkspaceKind::Worktree && path.is_dir() {
            let changes = self.git.status_changes(path).map_err(DomainError::from)?;
            if changes.uncommitted > 0 || changes.untracked > 0 {
                return Err(DomainError::new(
                    codes::WORKSPACE_DIRTY,
                    format!(
                        "{} has changed since it was shown clean: {} uncommitted and {} untracked change(s); look again before removing it",
                        row.path, changes.uncommitted, changes.untracked
                    ),
                )
                .into());
            }
        }
        let row = self.mark_removing(row)?;
        self.settle(&row, |_| self.check_removable(&row, waivers))?;
        Ok(views::workspace(&row))
    }

    /// First half of removing `selector`. Marks the row `removing` and runs every refusal
    /// check, so the handler can publish `workspace.removing` with the returned workspace
    /// before it calls [`Workspaces::finish_remove`]. A refusal returns the row to `ready`.
    /// Calling this again on a `removing` row resumes an interrupted removal.
    pub fn begin_remove(
        &self,
        selector: &str,
        waivers: RemovalWaivers,
    ) -> Result<Workspace, DomainError> {
        let _guard = self.lock_mutations();
        let row = self.resolve_workspace(selector)?;
        let row = self.mark_removing(row)?;
        self.settle(&row, |_| self.check_removable(&row, waivers))?;
        Ok(views::workspace(&row))
    }

    /// Returns workspace `id` from `removing` to `ready` after [`Workspaces::begin_remove`], when a step before git
    /// acts refuses the removal, such as a failed archive script (decision 0095). A row in any other state is left.
    pub fn cancel_remove(&self, id: &str) -> Result<(), DomainError> {
        let _guard = self.lock_mutations();
        if let Some(row) = self.store.get_workspace(id)?
            && row.state == StoreWorkspaceState::Removing
        {
            self.store
                .set_workspace_state(&row.id, StoreWorkspaceState::Ready)?;
        }
        Ok(())
    }

    /// Second half of removing workspace `id`. Runs the checks again, since the tree may have
    /// changed since [`Workspaces::begin_remove`], then removes the worktree (`--force` only
    /// when `force`), deletes the row, and deletes the branch the workspace created when git
    /// proves it merged. A folder workspace is forgotten and its files stay.
    ///
    /// A failure before git acts returns the row to `ready`. A failure after git has removed
    /// the worktree leaves it `removing`; calling [`Workspaces::finish_remove`] again completes
    /// the removal, because each step accepts a directory or registration that is already gone.
    pub fn finish_remove(
        &self,
        id: &str,
        waivers: RemovalWaivers,
    ) -> Result<WorkspaceRemoved, DomainError> {
        let _guard = self.lock_mutations();
        let row = self.store.get_workspace(id)?.ok_or_else(|| {
            DomainError::new(
                codes::WORKSPACE_NOT_FOUND,
                format!("no workspace with id {id:?}"),
            )
        })?;
        let row = self.mark_removing(row)?;
        self.settle(&row, |git_acted| {
            self.remove_marked(&row, waivers, git_acted)
        })
    }

    /// The second half after `mark_removing`: the checks, then git, then the row, then the branch.
    fn remove_marked(
        &self,
        row: &WorkspaceRow,
        waivers: RemovalWaivers,
        git_acted: &mut bool,
    ) -> Result<WorkspaceRemoved, DomainError> {
        let standing = self.check_removable(row, waivers)?;
        let project = self.project_of(row)?;
        let (branch_deleted, branch_kept_reason, directory_kept) = match row.kind {
            StoreWorkspaceKind::Worktree => {
                // An orphan is kept whole, so its branch stays with it.
                let orphan = standing == Standing::Orphan;
                let directory_kept = if orphan {
                    true
                } else {
                    self.remove_worktree(&project, row, waivers.force, git_acted)?
                };
                self.store.delete_workspace(&row.id)?;
                if orphan {
                    (
                        false,
                        Some("the worktree directory is kept".to_owned()),
                        true,
                    )
                } else {
                    let (deleted, reason) = self.delete_created_branch(&project, row);
                    (deleted, reason, directory_kept)
                }
            }
            StoreWorkspaceKind::Folder => {
                self.store.delete_workspace(&row.id)?;
                (false, None, Path::new(&row.path).is_dir())
            }
        };
        Ok(WorkspaceRemoved {
            workspace: views::workspace(row),
            branch_deleted,
            branch_kept_reason,
            directory_kept,
            removed_children: Vec::new(),
            archive: None,
        })
    }

    /// Writes `removing` before anything is deleted, and returns the row as it now is.
    fn mark_removing(&self, mut row: WorkspaceRow) -> Result<WorkspaceRow, DomainError> {
        if row.state != StoreWorkspaceState::Removing {
            self.store
                .set_workspace_state(&row.id, StoreWorkspaceState::Removing)?;
            row.state = StoreWorkspaceState::Removing;
        }
        Ok(row)
    }

    /// Runs one removal step on a row already marked `removing`, and settles the row on every
    /// error path here, so no step restores it by hand. Until git acts, nothing is removed and
    /// the row returns to `ready`. Once git has acted, the row stays `removing`: a new removal
    /// resumes it, and it still holds the work to finish.
    fn settle<T>(
        &self,
        row: &WorkspaceRow,
        step: impl FnOnce(&mut bool) -> Result<T, DomainError>,
    ) -> Result<T, DomainError> {
        let mut git_acted = false;
        match step(&mut git_acted) {
            Ok(value) => Ok(value),
            Err(error) => {
                if !git_acted {
                    // A failed restore leaves the row `removing`, which a new removal resumes.
                    let _ = self
                        .store
                        .set_workspace_state(&row.id, StoreWorkspaceState::Ready);
                }
                Err(error)
            }
        }
    }

    /// The refusals: the main worktree and a locked worktree always, a dirty one unless
    /// forced. Returns how the row was found, which the removal acts on.
    fn check_removable(
        &self,
        row: &WorkspaceRow,
        waivers: RemovalWaivers,
    ) -> Result<Standing, DomainError> {
        if row.kind == StoreWorkspaceKind::Folder {
            return Ok(Standing::Folder);
        }
        let project = self.project_of(row)?;
        let repo = Path::new(&project.path);
        let path = Path::new(&row.path);
        let key = paths::path_key(path);
        let entries = self.git.worktree_list(repo)?;
        if path.is_dir() && !self.is_recognised(&project, row)? {
            if !waivers.forget_orphan {
                return Err(DomainError::new(
                    codes::WORKSPACE_ORPHAN,
                    format!(
                        "{} is not a worktree git recognises; remove with forget_orphan to forget the row and keep the directory",
                        row.path
                    ),
                ));
            }
            return Ok(Standing::Orphan);
        }
        if entries
            .first()
            .is_some_and(|main| paths::path_key(&main.path) == key)
        {
            return Err(DomainError::new(
                codes::WORKSPACE_MAIN_WORKTREE,
                format!("{} is the main worktree and is never removed", row.path),
            ));
        }
        let registered = entries
            .iter()
            .any(|entry| paths::path_key(&entry.path) == key);
        let locked = entries
            .iter()
            .any(|entry| entry.locked.is_some() && paths::path_key(&entry.path) == key);
        if locked {
            return Err(DomainError::new(
                codes::WORKSPACE_LOCKED,
                format!("{} is locked; unlock it with git first", row.path),
            ));
        }
        // Only a registered worktree is deleted, so only its changes can be lost. git status does
        // not list ignored files, so they are counted apart and need their own waiver.
        if registered && path.is_dir() {
            let changes = self.git.status_changes(path)?;
            if !waivers.force && (changes.uncommitted > 0 || changes.untracked > 0) {
                return Err(DomainError::new(
                    codes::WORKSPACE_DIRTY,
                    format!(
                        "{} has {} uncommitted and {} untracked change(s); remove with force to discard them",
                        row.path, changes.uncommitted, changes.untracked
                    ),
                ));
            }
            if !waivers.discard_ignored && changes.ignored > 0 {
                return Err(DomainError::new(
                    codes::WORKSPACE_IGNORED_FILES,
                    format!(
                        "{} holds ignored files that removal would delete ({} ignored {} in git status; a directory counts once); remove with discard_ignored to delete them",
                        row.path,
                        changes.ignored,
                        if changes.ignored == 1 {
                            "entry"
                        } else {
                            "entries"
                        }
                    ),
                ));
            }
        }
        Ok(Standing::Worktree)
    }

    /// Whether git recognises the row's directory as its worktree: the worktree is registered,
    /// and git reports the directory as its own top. A directory whose .git file is gone, or one
    /// inside another repository, is not recognised. Git errors about the directory itself count
    /// as not recognised; a git that cannot run at all is still an error.
    fn is_recognised(&self, project: &ProjectRow, row: &WorkspaceRow) -> Result<bool, DomainError> {
        let repo = Path::new(&project.path);
        let path = Path::new(&row.path);
        let key = paths::path_key(path);
        let registered = self
            .git
            .worktree_list(repo)?
            .iter()
            .any(|entry| paths::path_key(&entry.path) == key);
        if !registered {
            return Ok(false);
        }
        match self.git.toplevel(path) {
            Ok(Some(top)) => Ok(paths::path_key(&top) == key),
            Ok(None) | Err(GitError::Failed { .. }) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    /// Removes the worktree git registers at the row's path; a registered worktree whose
    /// directory is already gone is still removed from git's records. A directory git does not
    /// register is never deleted: the row is forgotten and the directory stays. Returns whether
    /// a directory was kept.
    ///
    /// When git refuses the removal while it still registers the worktree, nothing was removed,
    /// so the row goes back to `ready` and the git error is returned.
    fn remove_worktree(
        &self,
        project: &ProjectRow,
        row: &WorkspaceRow,
        force: bool,
        git_acted: &mut bool,
    ) -> Result<bool, DomainError> {
        let repo = Path::new(&project.path);
        let path = Path::new(&row.path);
        let key = paths::path_key(path);
        let registered = self
            .git
            .worktree_list(repo)?
            .iter()
            .any(|entry| paths::path_key(&entry.path) == key);
        if !registered {
            return Ok(path.is_dir());
        }
        *git_acted = true;
        if let Err(error) = self.git.worktree_remove(repo, path, force) {
            // A refusal that leaves git's record in place removed nothing.
            let still_registered = self.git.worktree_list(repo).is_ok_and(|entries| {
                entries
                    .iter()
                    .any(|entry| paths::path_key(&entry.path) == key)
            });
            *git_acted = !still_registered;
            return Err(error.into());
        }
        Ok(false)
    }

    /// Deletes the branch a workspace created. Returns whether it was deleted and, when it was
    /// kept, why. A branch this workspace did not create is never touched.
    fn delete_created_branch(
        &self,
        project: &ProjectRow,
        row: &WorkspaceRow,
    ) -> (bool, Option<String>) {
        match row.branch.as_deref() {
            Some(branch) if row.created_branch => {
                match self
                    .git
                    .delete_branch_if_merged(Path::new(&project.path), branch)
                {
                    Ok(true) => (true, None),
                    Ok(false) => (false, Some("not merged".to_owned())),
                    Err(error) => (false, Some(format!("git could not delete it: {error}"))),
                }
            }
            _ => (false, None),
        }
    }

    fn project_of(&self, row: &WorkspaceRow) -> Result<ProjectRow, DomainError> {
        self.store.get_project(&row.project_id)?.ok_or_else(|| {
            DomainError::new(
                codes::PROJECT_NOT_FOUND,
                format!("the project of workspace {} is gone", row.name),
            )
        })
    }
}
