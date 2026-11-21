//! Removing workspaces (ADR 0008): the clean path first. Refusals, kept directories and branch
//! deletion follow in later changes.

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
    /// First half of removing `selector`: marks the row `removing`, so the handler can publish
    /// `workspace.removing` before [`Workspaces::finish_remove`]. Calling it again on a
    /// `removing` row resumes an interrupted removal.
    pub fn begin_remove(&self, selector: &str, _force: bool) -> Result<Workspace, DomainError> {
        let _guard = self.lock_mutations();
        let row = self.resolve_workspace(selector)?;
        let row = self.mark_removing(row)?;
        Ok(views::workspace(&row))
    }

    /// Second half of removing workspace `id`: removes the worktree git registers (`--force` only
    /// when `force`), then deletes the row. A folder workspace is forgotten and its files stay.
    pub fn finish_remove(&self, id: &str, force: bool) -> Result<WorkspaceRemoved, DomainError> {
        let _guard = self.lock_mutations();
        let row = self.store.get_workspace(id)?.ok_or_else(|| {
            DomainError::new(
                codes::WORKSPACE_NOT_FOUND,
                format!("no workspace with id {id:?}"),
            )
        })?;
        let row = self.mark_removing(row)?;
        if row.kind == StoreWorkspaceKind::Worktree {
            let project = self.project_of(&row)?;
            self.remove_worktree(&project, &row, force)?;
        }
        self.store.delete_workspace(&row.id)?;
        Ok(WorkspaceRemoved {
            workspace: views::workspace(&row),
            branch_deleted: false,
            branch_kept_reason: None,
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

    /// Removes the worktree git registers at the row's path. A directory git does not register is
    /// not touched.
    fn remove_worktree(
        &self,
        project: &ProjectRow,
        row: &WorkspaceRow,
        force: bool,
    ) -> Result<(), DomainError> {
        let repo = Path::new(&project.path);
        let path = Path::new(&row.path);
        let key = paths::path_key(path);
        let registered = self
            .git
            .worktree_list(repo)?
            .iter()
            .any(|entry| paths::path_key(&entry.path) == key);
        if registered {
            self.git.worktree_remove(repo, path, force)?;
        }
        Ok(())
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
