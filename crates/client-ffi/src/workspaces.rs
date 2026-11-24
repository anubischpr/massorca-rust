//! The workspaces of a host, read once (decision 0080, screen M-WS-01).

use massorca_protocol::methods::workspaces::{Workspace, WorkspaceListParams};

use crate::error::FfiError;
use crate::massorca::{Host, run_on};
use crate::wire::word;

/// A workspace as the app lists it: a worktree of a git project, or a folder.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WorkspaceSummary {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub path: String,
    /// `worktree` or `folder`; `unknown` for a kind this build does not know.
    pub kind: String,
    /// The checked-out branch, when the workspace has one.
    pub branch: Option<String>,
    /// `ready` or `removing`; `unknown` for a state this build does not know.
    pub state: String,
    /// The workspace's directory is gone.
    pub missing: bool,
}

impl From<Workspace> for WorkspaceSummary {
    fn from(workspace: Workspace) -> Self {
        Self {
            id: workspace.id,
            project_id: workspace.project_id,
            name: workspace.name,
            path: workspace.path,
            kind: word(workspace.kind),
            branch: workspace.branch,
            state: word(workspace.state),
            missing: workspace.missing,
        }
    }
}

#[uniffi::export]
impl Host {
    /// The workspaces of every project, or only those of `project` (its id).
    pub async fn workspaces(
        &self,
        project: Option<String>,
    ) -> Result<Vec<WorkspaceSummary>, FfiError> {
        let client = self.client.clone();
        let name = self.name.clone();
        run_on(&self.runtime, async move {
            let list = client
                .workspace_list(&WorkspaceListParams { project })
                .await
                .map_err(|error| FfiError::from_client(error, &name))?;
            Ok(list
                .workspaces
                .into_iter()
                .map(WorkspaceSummary::from)
                .collect())
        })
        .await
    }
}
