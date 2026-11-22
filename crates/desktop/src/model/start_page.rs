//! The workspace start page as plain data (D-AG-01, J3): what the Canvas shows for a workspace with
//! no session open. The facts come from the records the service reported; nothing here starts a
//! session. Free of GPUI, so the rules test without a window.

use massorca_protocol::methods::workspaces::WorkspaceState;

use super::service_view::LOCAL_HOST;
use crate::service::StoreState;

/// Why the page's actions that need sessions or git are off.
pub const SESSIONS_LATER: &str = "Agent sessions come later.";
pub const GIT_LATER: &str = "Git operations come later.";

/// One workspace as the start page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceStart {
    pub id: String,
    pub name: String,
    pub project_id: String,
    /// The project's name, or a plain word when the service no longer lists the project.
    pub project: String,
    pub host: &'static str,
    pub path: String,
    pub branch: Option<String>,
    pub state: &'static str,
    /// The folder is not on disk any more. The page says so and changes nothing.
    pub missing: bool,
}

/// The page for `workspace_id`, or `None` when the store does not hold that workspace.
pub fn workspace_start(state: &StoreState, workspace_id: &str) -> Option<WorkspaceStart> {
    let workspace = state
        .workspaces
        .iter()
        .find(|workspace| workspace.id == workspace_id)?;
    let project = state
        .projects
        .iter()
        .find(|project| project.id == workspace.project_id)
        .map_or_else(
            || "Unknown project".to_owned(),
            |project| project.name.clone(),
        );
    Some(WorkspaceStart {
        id: workspace.id.clone(),
        name: workspace.name.clone(),
        project_id: workspace.project_id.clone(),
        project,
        host: LOCAL_HOST,
        path: workspace.path.clone(),
        branch: workspace.branch.clone(),
        state: state_text(workspace.state),
        missing: workspace.missing,
    })
}

/// The word a workspace state shows (D-WS-01). A state this build does not know says so.
pub fn state_text(state: WorkspaceState) -> &'static str {
    match state {
        WorkspaceState::Ready => "Ready",
        WorkspaceState::Removing => "Removing",
        WorkspaceState::Unknown => "Unknown state",
    }
}

/// The workspace a project choice lands on: the project's first workspace in creation order.
pub fn first_workspace_of(state: &StoreState, project_id: &str) -> Option<String> {
    state
        .workspaces
        .iter()
        .filter(|workspace| workspace.project_id == project_id)
        .min_by_key(|workspace| workspace.created_at_ms)
        .map(|workspace| workspace.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fixtures::{project, workspace};

    fn store() -> StoreState {
        StoreState {
            projects: vec![project("p1", "alpha"), project("p2", "beta")],
            workspaces: vec![
                workspace("w2", "p1", "later", 20),
                workspace("w1", "p1", "first", 10),
                workspace("w3", "p2", "other", 5),
            ],
            ..StoreState::default()
        }
    }

    #[test]
    fn the_page_shows_the_workspace_facts_the_service_reported() {
        let page = workspace_start(&store(), "w1").expect("the workspace is in the store");
        assert_eq!(page.name, "first");
        assert_eq!(page.project, "alpha");
        assert_eq!(page.host, "This PC");
        assert_eq!(page.path, "/repos/first");
        assert_eq!(page.branch.as_deref(), Some("feat/first"));
        assert_eq!(page.state, "Ready");
        assert!(!page.missing);
    }

    #[test]
    fn a_workspace_the_store_does_not_hold_has_no_page() {
        assert_eq!(workspace_start(&store(), "nope"), None);
    }

    #[test]
    fn a_missing_folder_and_a_project_no_longer_listed_are_said_plainly() {
        let mut state = store();
        state.workspaces[1].missing = true;
        state.projects.clear();
        let page = workspace_start(&state, "w1").expect("the workspace is in the store");
        assert!(page.missing);
        assert_eq!(page.project, "Unknown project");
    }

    #[test]
    fn every_workspace_state_has_its_own_word() {
        assert_eq!(state_text(WorkspaceState::Ready), "Ready");
        assert_eq!(state_text(WorkspaceState::Removing), "Removing");
        assert_eq!(state_text(WorkspaceState::Unknown), "Unknown state");
    }

    #[test]
    fn a_project_choice_lands_on_its_first_workspace_by_creation_time() {
        assert_eq!(first_workspace_of(&store(), "p1").as_deref(), Some("w1"));
        assert_eq!(first_workspace_of(&store(), "p2").as_deref(), Some("w3"));
        assert_eq!(first_workspace_of(&store(), "p9"), None);
    }
}
