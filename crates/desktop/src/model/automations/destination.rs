//! The destination an automation shows on the page (decision 0127): where its runs work, as one line.

use massorca_protocol::methods::automations::{Automation, AutomationDestinationKind};

/// One line for the page: the project's own place, a new worktree per run, or the workspace by name while it exists.
pub fn destination_summary(automation: &Automation) -> String {
    let destination = &automation.destination;
    match destination.kind {
        AutomationDestinationKind::ProjectDefault => "Runs in the project's own place".to_owned(),
        AutomationDestinationKind::ProjectWorktreePerRun => match &destination.base_branch {
            Some(branch) => format!("New worktree per run from {branch}"),
            None => "New worktree per run".to_owned(),
        },
        AutomationDestinationKind::ExistingWorkspace => {
            match (&destination.workspace_name, &destination.workspace) {
                (Some(name), _) => format!("Runs in workspace {name}"),
                (None, Some(_)) => "Its workspace no longer exists".to_owned(),
                (None, None) => "No workspace chosen".to_owned(),
            }
        }
        AutomationDestinationKind::Unknown => "A destination this version does not know".to_owned(),
    }
}
