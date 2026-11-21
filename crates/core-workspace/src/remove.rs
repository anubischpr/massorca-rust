//! Removing workspaces (ADR 0008): refusals first, then git, then the row, then the branch.

use std::path::Path;

use massorca_core_store::{
    ProjectRow, WorkspaceKind as StoreWorkspaceKind, WorkspaceRow,
    WorkspaceState as StoreWorkspaceState,
};
use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceRemoved};

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

impl Workspaces {
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
        self.check_or_restore(&row, waivers)?;
        Ok(views::workspace(&row))
    }

    /// Second half of removing workspace `id`. Runs the checks again, since the tree may have
    /// changed since [`Workspaces::begin_remove`], then removes the worktree (`--force` only
    /// when `force`), deletes the row, and deletes the branch the workspace created when git
    /// proves it merged. A folder workspace is forgotten and its files stay.
    ///
    /// A failure after the row is marked `removing` leaves it that way; calling
    /// [`Workspaces::begin_remove`] again finishes the job.
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
        self.check_or_restore(&row, waivers)?;
        let project = self.project_of(&row)?;
        let (branch_deleted, branch_kept_reason, directory_kept) = match row.kind {
            StoreWorkspaceKind::Worktree => {
                let directory_kept = self.remove_worktree(&project, &row, waivers.force)?;
                self.store.delete_workspace(&row.id)?;
                let (deleted, reason) = self.delete_created_branch(&project, &row);
                (deleted, reason, directory_kept)
            }
            StoreWorkspaceKind::Folder => {
                self.store.delete_workspace(&row.id)?;
                (false, None, Path::new(&row.path).is_dir())
            }
        };
        Ok(WorkspaceRemoved {
            workspace: views::workspace(&row),
            branch_deleted,
            branch_kept_reason,
            directory_kept,
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

    /// Runs the checks. The checks touch no file, so any failure, a refusal or git unable to run,
    /// puts the row back to `ready`. Only a failure after git has acted leaves it `removing`.
    fn check_or_restore(
        &self,
        row: &WorkspaceRow,
        waivers: RemovalWaivers,
    ) -> Result<(), DomainError> {
        match self.check_removable(row, waivers) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.store
                    .set_workspace_state(&row.id, StoreWorkspaceState::Ready)?;
                Err(error)
            }
        }
    }

    /// The refusals: the main worktree and a locked worktree always, a dirty one unless
    /// forced. A folder workspace has no git state to check.
    fn check_removable(
        &self,
        row: &WorkspaceRow,
        waivers: RemovalWaivers,
    ) -> Result<(), DomainError> {
        if row.kind == StoreWorkspaceKind::Folder {
            return Ok(());
        }
        let project = self.project_of(row)?;
        let repo = Path::new(&project.path);
        let path = Path::new(&row.path);
        let key = paths::path_key(path);
        let entries = self.git.worktree_list(repo)?;
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
                        "{} has {} ignored entr{} that removal would delete; remove with discard_ignored to delete them",
                        row.path,
                        changes.ignored,
                        if changes.ignored == 1 { "y" } else { "ies" }
                    ),
                ));
            }
        }
        Ok(())
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
        if let Err(error) = self.git.worktree_remove(repo, path, force) {
            let still_registered = self.git.worktree_list(repo).is_ok_and(|entries| {
                entries
                    .iter()
                    .any(|entry| paths::path_key(&entry.path) == key)
            });
            if still_registered {
                self.store
                    .set_workspace_state(&row.id, StoreWorkspaceState::Ready)?;
            }
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
