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

impl Workspaces {
    /// First half of removing `selector`. Marks the row `removing` and runs every refusal
    /// check, so the handler can publish `workspace.removing` with the returned workspace
    /// before it calls [`Workspaces::finish_remove`]. A refusal returns the row to `ready`.
    /// Calling this again on a `removing` row resumes an interrupted removal.
    pub fn begin_remove(&self, selector: &str, force: bool) -> Result<Workspace, DomainError> {
        let _guard = self.lock_mutations();
        let row = self.resolve_workspace(selector)?;
        let row = self.mark_removing(row)?;
        self.check_or_restore(&row, force)?;
        Ok(views::workspace(&row))
    }

    /// Second half of removing workspace `id`. Runs the checks again, since the tree may have
    /// changed since [`Workspaces::begin_remove`], then removes the worktree (`--force` only
    /// when `force`), deletes the row, and deletes the branch the workspace created when git
    /// proves it merged. A folder workspace is forgotten and its files stay.
    ///
    /// A failure after the row is marked `removing` leaves it that way; calling
    /// [`Workspaces::begin_remove`] again finishes the job.
    pub fn finish_remove(&self, id: &str, force: bool) -> Result<WorkspaceRemoved, DomainError> {
        let _guard = self.lock_mutations();
        let row = self.store.get_workspace(id)?.ok_or_else(|| {
            DomainError::new(
                codes::WORKSPACE_NOT_FOUND,
                format!("no workspace with id {id:?}"),
            )
        })?;
        let row = self.mark_removing(row)?;
        self.check_or_restore(&row, force)?;
        let project = self.project_of(&row)?;
        let (branch_deleted, branch_kept_reason, directory_kept) = match row.kind {
            StoreWorkspaceKind::Worktree => {
                let directory_kept = self.remove_worktree(&project, &row, force)?;
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

    /// Runs the checks. A refusal puts the row back to `ready`, because no file was touched;
    /// any other failure leaves it `removing` so the removal can be resumed.
    fn check_or_restore(&self, row: &WorkspaceRow, force: bool) -> Result<(), DomainError> {
        match self.check_removable(row, force) {
            Ok(()) => Ok(()),
            Err(error) if is_refusal(error.code()) => {
                self.store
                    .set_workspace_state(&row.id, StoreWorkspaceState::Ready)?;
                Err(error)
            }
            Err(error) => Err(error),
        }
    }

    /// The refusals: the main worktree and a locked worktree always, a dirty one unless
    /// forced. A folder workspace has no git state to check.
    fn check_removable(&self, row: &WorkspaceRow, force: bool) -> Result<(), DomainError> {
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
        // Only a registered worktree is deleted, so only its changes can be lost.
        if registered && !force && path.is_dir() && self.git.status_dirty(path)? {
            return Err(DomainError::new(
                codes::WORKSPACE_DIRTY,
                format!(
                    "{} has uncommitted or untracked changes; remove with force to discard them",
                    row.path
                ),
            ));
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

/// The refusals, which leave the workspace as it was before the removal began.
fn is_refusal(code: &str) -> bool {
    matches!(
        code,
        codes::WORKSPACE_DIRTY | codes::WORKSPACE_LOCKED | codes::WORKSPACE_MAIN_WORKTREE
    )
}
