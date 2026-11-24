//! The start page with no workspace (D-AG-01, J3), where "+ Create" lands. Send creates a workspace
//! in the chosen project, named from the prompt, then starts the agent in it. Free of GPUI.

use massorca_protocol::methods::agents::AgentStartParams;
use massorca_protocol::methods::projects::Project;
use massorca_protocol::methods::workspaces::WorkspaceCreateParams;

use super::start_page::start_params;

/// The key of the page's composer among the workspace start composers; no workspace id is empty.
pub const HOME_START: &str = "";

/// Why "No project" is off until the service can start an agent without a workspace (A95).
pub const NO_PROJECT_NEEDS_SERVICE: &str = "Needs a newer service.";

/// Send's reason while there is no project to create the workspace in.
pub const ADD_A_PROJECT: &str = "Add a project to start a session.";

/// The project button's text while there is none.
pub const CHOOSE_A_PROJECT: &str = "Choose a project";

/// How many of the prompt's words name the workspace, and the longest name they make.
const SEED_WORDS: usize = 5;
const SEED_MAX: usize = 40;

/// A workspace name from the prompt's first words: lowercase ASCII letters and digits joined by
/// `-`. Empty when no word survives, which asks the service for a generated name.
pub fn workspace_name_seed(prompt: &str) -> String {
    let mut name = String::new();
    let words = prompt
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .take(SEED_WORDS);
    for word in words {
        let word = word.to_ascii_lowercase();
        let extra = usize::from(!name.is_empty()) + word.len();
        if name.len() + extra > SEED_MAX {
            break;
        }
        if !name.is_empty() {
            name.push('-');
        }
        name.push_str(&word);
    }
    name
}

/// The project the page creates in: the chosen one while the service lists it, else the first.
pub fn home_project<'a>(projects: &'a [Project], chosen: Option<&str>) -> Option<&'a Project> {
    chosen
        .and_then(|id| projects.iter().find(|project| project.id == id))
        .or_else(|| projects.first())
}

/// The `workspace.create` that Send asks for: a new worktree in `project`, named from the prompt.
pub fn create_params(project: &str, prompt: &str) -> WorkspaceCreateParams {
    WorkspaceCreateParams {
        project: project.to_owned(),
        name: workspace_name_seed(prompt),
        branch: None,
        base: None,
        existing_branch: false,
        refresh_base: false,
        path: None,
        request_id: None,
        no_setup: false,
    }
}

/// The `agent.start` in the new workspace. A blank prompt starts the agent with none.
pub fn home_start_params(
    workspace: &str,
    kind: &str,
    prompt: &str,
    permission_mode: Option<String>,
) -> AgentStartParams {
    let mut params = start_params(workspace, kind, prompt, permission_mode);
    if prompt.trim().is_empty() {
        params.prompt = None;
    }
    params
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fixtures::project;

    #[test]
    fn the_name_is_the_prompts_first_words_slug_safe() {
        assert_eq!(
            workspace_name_seed("Fix the login bug"),
            "fix-the-login-bug"
        );
        assert_eq!(
            workspace_name_seed("  Add OAuth2: Google, GitHub & more providers now"),
            "add-oauth2-google-github-more"
        );
        assert_eq!(workspace_name_seed("¿Qué tal?"), "qu-tal");
    }

    #[test]
    fn a_blank_or_wordless_prompt_asks_for_a_generated_name() {
        assert_eq!(workspace_name_seed(""), "");
        assert_eq!(workspace_name_seed("  ¿¡! "), "");
    }

    #[test]
    fn the_name_stops_before_a_word_that_would_pass_the_limit() {
        let name = workspace_name_seed(
            "refactor everything inside the extraordinarilylongmodulenameherereally",
        );
        assert_eq!(name, "refactor-everything-inside-the");
        assert!(name.len() <= SEED_MAX);
    }

    #[test]
    fn the_chosen_project_wins_while_listed_and_the_first_stands_in_otherwise() {
        let projects = [project("p1", "alpha"), project("p2", "beta")];
        assert_eq!(
            home_project(&projects, Some("p2")).map(|p| p.id.as_str()),
            Some("p2")
        );
        assert_eq!(
            home_project(&projects, Some("gone")).map(|p| p.id.as_str()),
            Some("p1")
        );
        assert_eq!(
            home_project(&projects, None).map(|p| p.id.as_str()),
            Some("p1")
        );
        assert!(home_project(&[], None).is_none());
    }

    #[test]
    fn the_create_call_names_the_workspace_from_the_prompt() {
        let params = create_params("p1", "fix the login");
        assert_eq!(params.project, "p1");
        assert_eq!(params.name, "fix-the-login");
        assert_eq!(params.branch, None);
        assert!(!params.existing_branch);
    }

    #[test]
    fn a_blank_prompt_starts_the_agent_with_no_prompt() {
        assert_eq!(home_start_params("w9", "claude", "  ", None).prompt, None);
        let params = home_start_params("w9", "claude", "fix it", Some("plan".into()));
        assert_eq!(params.workspace.as_deref(), Some("w9"));
        assert_eq!(params.prompt.as_deref(), Some("fix it"));
        assert_eq!(params.permission_mode.as_deref(), Some("plan"));
    }
}
