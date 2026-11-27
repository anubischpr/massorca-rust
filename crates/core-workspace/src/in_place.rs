//! The project's own checkout as its in-place workspace (decision 0112): one row for it, and no worktree, branch or file
//! is made or removed for it.

use std::path::Path;

use massorca_core_store::{
    ProjectKind as StoreKind, ProjectRow, WorkspaceKind as StoreWorkspaceKind, WorkspaceRow,
    WorkspaceState as StoreWorkspaceState, new_id, now_ms,
};
use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};

use crate::create::{Created, invalid, refused_write};
use crate::error::DomainError;
use crate::names;
use crate::paths;
use crate::views;
use crate::workspaces::Workspaces;

impl Workspaces {
    /// The row as a workspace. An in-place workspace's branch is read from its checkout now, since the person may switch
    /// it; a created worktree keeps the branch it was made on.
    pub(crate) fn view(&self, row: &WorkspaceRow) -> Workspace {
        let mut workspace = views::workspace(row);
        if row.in_place {
            workspace.branch = self.git.current_branch(Path::new(&row.path)).ok().flatten();
        }
        workspace
    }

    /// Registers the project's own checkout as its in-place workspace, or returns the one it already has. The name is
    /// the project's unless one is given. Called under the mutation lock.
    pub(crate) fn create_in_place(
        &self,
        project: &ProjectRow,
        params: &WorkspaceCreateParams,
    ) -> Result<Created, DomainError> {
        if params.branch.is_some()
            || params.base.is_some()
            || params.existing_branch
            || params.refresh_base
            || params.path.is_some()
            || params.unique_name
            || params.name_generated
        {
            return Err(invalid(
                "in_place takes none of branch, base, existing_branch, refresh_base, path, unique_name or name_generated: the service never switches the user's checkout",
            ));
        }
        let directory = paths::existing_directory(Path::new(&project.path))?;
        let key = paths::path_key(&directory);
        let existing = match project.kind {
            StoreKind::Git => self
                .store
                .list_workspaces(Some(&project.id))?
                .into_iter()
                .find(|row| row.in_place),
            StoreKind::Folder => self
                .store
                .find_workspace_by_path_key(&key)?
                .filter(|row| row.project_id == project.id),
        };
        if let Some(row) = existing {
            return Ok(Created {
                workspace: self.view(&row),
                repeated: true,
            });
        }
        let typed = if params.name.trim().is_empty() {
            project.name.as_str()
        } else {
            params.name.as_str()
        };
        let name = names::clean_workspace_name(typed)?;
        if self
            .store
            .find_workspace_by_name(&project.id, &name)?
            .is_some()
        {
            return Err(DomainError::new(
                codes::WORKSPACE_NAME_TAKEN,
                format!(
                    "project {:?} already has a workspace named {:?}",
                    project.name, name
                ),
            ));
        }
        let kind = match project.kind {
            StoreKind::Git => StoreWorkspaceKind::Worktree,
            StoreKind::Folder => StoreWorkspaceKind::Folder,
        };
        let row = WorkspaceRow {
            id: new_id(),
            project_id: project.id.clone(),
            kind,
            name,
            path: paths::path_text(&directory)?,
            path_key: key,
            branch: None,
            base_ref: None,
            created_branch: false,
            state: StoreWorkspaceState::Ready,
            request_id: params.request_id.clone(),
            created_at: now_ms(),
            comment: None,
            name_generated: false,
            in_place: true,
        };
        self.store.insert_workspace(&row).map_err(refused_write)?;
        Ok(Created {
            workspace: self.view(&row),
            repeated: false,
        })
    }
}
