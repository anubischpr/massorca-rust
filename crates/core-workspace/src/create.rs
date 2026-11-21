//! Creating workspaces (ADR 0008): validates the request and the names first.

use massorca_core_store::ProjectKind as StoreKind;
use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};

use crate::error::DomainError;
use crate::names;
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
            StoreKind::Folder => Err(not_yet("folder")),
        }
    }
}

fn invalid(message: &str) -> DomainError {
    DomainError::new(codes::REQUEST_INVALID_PARAMS, message)
}

/// Refuses a kind of workspace whose creation has not landed yet.
fn not_yet(kind: &str) -> DomainError {
    DomainError::new(
        codes::REQUEST_INVALID_PARAMS,
        format!("{kind} workspaces are not available yet"),
    )
}
