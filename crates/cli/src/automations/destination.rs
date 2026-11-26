//! The destination of `massorca automations create|edit` (decision 0127): the flags, and the default of a create, which
//! is the MassOrca workspace the current directory is in, as `workspace current` finds it.

use massorca_client::Client;
use massorca_protocol::codes;
use massorca_protocol::methods::automations::{AutomationDestination, AutomationDestinationKind};
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCurrentParams};

use crate::cwd_path::PathRules;
use crate::error::{CliError, exit};

/// The destination the flags name: an existing workspace, or a new worktree per run from `base_branch`.
pub fn from_flags(
    workspace: Option<String>,
    new_worktree: bool,
    base_branch: Option<String>,
) -> Option<AutomationDestination> {
    if let Some(id) = workspace {
        return Some(AutomationDestination {
            kind: AutomationDestinationKind::ExistingWorkspace,
            workspace: Some(id),
            ..AutomationDestination::default()
        });
    }
    new_worktree.then(|| AutomationDestination {
        kind: AutomationDestinationKind::ProjectWorktreePerRun,
        base_branch,
        ..AutomationDestination::default()
    })
}

/// The destination a workspace stands for: every run goes there.
pub fn existing(workspace: &Workspace) -> AutomationDestination {
    AutomationDestination {
        kind: AutomationDestinationKind::ExistingWorkspace,
        workspace: Some(workspace.id.clone()),
        ..AutomationDestination::default()
    }
}

/// The MassOrca workspace the current directory is in, if one resolves. A remote host cannot see this computer's
/// directory, so there is none then.
pub async fn enclosing(client: &Client, paths: PathRules) -> Result<Option<Workspace>, CliError> {
    if paths.is_remote() {
        return Ok(None);
    }
    let current = std::env::current_dir().map_err(|error| {
        CliError::new(
            "usage.invalid_path",
            format!("cannot read the current directory: {error}"),
            exit::USAGE,
        )
    })?;
    let path = paths.path(&current)?;
    match client
        .workspace_current(&WorkspaceCurrentParams { path })
        .await
    {
        Ok(workspace) => Ok(Some(workspace)),
        Err(error) => {
            let error = CliError::from(error);
            if error.code == codes::WORKSPACE_NOT_FOUND {
                Ok(None)
            } else {
                Err(error)
            }
        }
    }
}
