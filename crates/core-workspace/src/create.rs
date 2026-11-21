//! Creating workspaces (ADR 0008): a registration of an existing directory in a folder project.

use std::path::Path;

use massorca_core_store::{
    ProjectKind as StoreKind, ProjectRow, StoreError, WorkspaceKind as StoreWorkspaceKind,
    WorkspaceRow, WorkspaceState as StoreWorkspaceState, new_id, now_ms,
};
use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};

use crate::error::DomainError;
use crate::names;
use crate::paths;
use crate::views;
use crate::workspaces::Workspaces;

impl Workspaces {
    /// Creates a worktree (git project) or registers a folder (folder project). A repeated
    /// `request_id` returns the workspace the first call stored, whatever its other fields.
    pub fn create_workspace(
        &self,
        params: WorkspaceCreateParams,
    ) -> Result<Workspace, DomainError> {
        let _guard = self.lock_mutations();
        if let Some(request_id) = params.request_id.as_deref() {
            if request_id.is_empty() {
                return Err(invalid("request id must not be empty"));
            }
            if let Some(row) = self.store.find_workspace_by_request_id(request_id)? {
                return Ok(views::workspace(&row));
            }
        }
        let project = self.resolve_project(&params.project)?;
        names::workspace_name(&params.name)?;
        if self
            .store
            .find_workspace_by_name(&project.id, &params.name)?
            .is_some()
        {
            return Err(DomainError::new(
                codes::WORKSPACE_NAME_TAKEN,
                format!(
                    "project {:?} already has a workspace named {:?}",
                    project.name, params.name
                ),
            ));
        }
        match project.kind {
            StoreKind::Git => Err(not_yet("worktree")),
            StoreKind::Folder => self.create_folder(&project, &params),
        }
    }

    /// Registers an existing directory: the project's by default. Creates no files.
    fn create_folder(
        &self,
        project: &ProjectRow,
        params: &WorkspaceCreateParams,
    ) -> Result<Workspace, DomainError> {
        if params.branch.is_some() || params.base.is_some() || params.existing_branch {
            return Err(invalid(
                "branch, base and existing_branch apply only to worktrees",
            ));
        }
        let directory = match params.path.as_deref() {
            Some(text) => paths::existing_directory(&paths::absolute(text)?)?,
            None => paths::existing_directory(Path::new(&project.path))?,
        };
        let row = WorkspaceRow {
            id: new_id(),
            project_id: project.id.clone(),
            kind: StoreWorkspaceKind::Folder,
            name: params.name.clone(),
            path: paths::path_text(&directory)?,
            path_key: paths::path_key(&directory),
            branch: None,
            base_ref: None,
            created_branch: false,
            state: StoreWorkspaceState::Ready,
            request_id: params.request_id.clone(),
            created_at: now_ms(),
        };
        self.store.insert_workspace(&row).map_err(refused_insert)?;
        Ok(views::workspace(&row))
    }
}

fn invalid(message: &str) -> DomainError {
    DomainError::new(codes::REQUEST_INVALID_PARAMS, message)
}

/// The domain meaning of a row insert that the store refused.
fn refused_insert(error: StoreError) -> DomainError {
    match error {
        StoreError::Conflict { constraint } if constraint.contains("path_key") => DomainError::new(
            codes::WORKSPACE_PATH_TAKEN,
            "that path was claimed by another workspace",
        ),
        StoreError::Conflict { constraint } if constraint.contains("name") => DomainError::new(
            codes::WORKSPACE_NAME_TAKEN,
            "that name was claimed by another workspace",
        ),
        other => other.into(),
    }
}

/// Refuses a kind of workspace whose creation has not landed yet.
fn not_yet(kind: &str) -> DomainError {
    DomainError::new(
        codes::REQUEST_INVALID_PARAMS,
        format!("{kind} workspaces are not available yet"),
    )
}
