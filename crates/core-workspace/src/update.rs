//! `workspace.update`: a workspace's name and comment. The directory and the branch keep the names they were made with.

use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceUpdateParams};

use crate::create::refused_write;
use crate::error::DomainError;
use crate::names;
use crate::workspaces::Workspaces;

impl Workspaces {
    /// Changes the name and comment `params` names. A field it leaves out keeps its value, and an empty comment clears
    /// it. Refuses a request that changes nothing, a name the naming rules refuse, and a name another workspace holds.
    pub fn update_workspace(
        &self,
        params: WorkspaceUpdateParams,
    ) -> Result<Workspace, DomainError> {
        if params.name.is_none() && params.comment.is_none() {
            return Err(DomainError::new(
                codes::REQUEST_INVALID_PARAMS,
                "nothing to update: pass a name or a comment",
            ));
        }
        let _guard = self.lock_mutations();
        let mut row = self.resolve_workspace(&params.workspace)?;
        // A name a person typed is theirs, so it is never renamed for them again.
        let (name, name_generated) = match &params.name {
            Some(typed) => (names::clean_workspace_name(typed)?, false),
            None => (row.name.clone(), row.name_generated),
        };
        let comment = match params.comment {
            Some(text) if text.is_empty() => None,
            Some(text) => Some(text),
            None => row.comment.clone(),
        };
        self.store
            .update_workspace(&row.id, &name, comment.as_deref(), name_generated)
            .map_err(refused_write)?;
        row.name = name;
        row.comment = comment;
        row.name_generated = name_generated;
        Ok(self.view(&row))
    }
}
