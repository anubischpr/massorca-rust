//! The start page with no workspace (D-AG-01, J3), where "+ Create" lands. Send creates a workspace
//! in the chosen project, named from the prompt, then starts the agent in it. Free of GPUI.

use massorca_protocol::methods::agents::AgentStartParams;
use massorca_protocol::methods::projects::Project;
use massorca_protocol::methods::workspaces::WorkspaceCreateParams;

use super::start_page::{StartOptions, start_params};

/// The key of the page's composer among the workspace start composers; no workspace id is empty.
pub const HOME_START: &str = "";

/// Why a start-page control is off until the service can take it.
pub const NEEDS_NEWER_SERVICE: &str = "Needs a newer service.";

/// Why "No project" is off until the service can start an agent without a workspace (A95).
pub const NO_PROJECT_NEEDS_SERVICE: &str = NEEDS_NEWER_SERVICE;

/// Send's reason while "Use an existing branch" has no branch name to reuse.
pub const NAME_THE_BRANCH: &str = "Name the branch to reuse.";

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

/// The Advanced section's fields as Send reads them. Blank text sends nothing for its field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceDetails {
    /// Blank takes the prompt's first words, or the service's generated name.
    pub name: String,
    /// The new branch's name, or the existing branch's when `existing_branch` is set.
    pub branch: String,
    /// The base ref the new branch starts from.
    pub base: String,
    pub existing_branch: bool,
}

impl WorkspaceDetails {
    /// Why Send is off: a branch to reuse must be named.
    pub fn send_block(&self) -> Option<&'static str> {
        (self.existing_branch && non_empty(&self.branch).is_none()).then_some(NAME_THE_BRANCH)
    }
}

/// The branch pill's two labels: the branch Send creates or reuses, and where a new worktree starts (D-AG-01).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchPill {
    pub label: String,
    pub detail: String,
}

/// The branch pill's labels: a new worktree from the base the menu names, else the project's base, else the default
/// branch; or the branch the menu reuses.
pub fn branch_pill(details: &WorkspaceDetails, project_base: Option<&str>) -> BranchPill {
    if details.existing_branch {
        return BranchPill {
            label: non_empty(&details.branch).unwrap_or_else(|| "Existing branch".to_owned()),
            detail: "existing branch".to_owned(),
        };
    }
    let base = non_empty(&details.base)
        .or_else(|| project_base.and_then(non_empty))
        .unwrap_or_else(|| "the default branch".to_owned());
    BranchPill {
        label: "New worktree".to_owned(),
        detail: format!("from {base}"),
    }
}

/// The `workspace.create` that Send asks for: a new worktree in `project`.
pub fn create_params(
    project: &str,
    prompt: &str,
    details: &WorkspaceDetails,
) -> WorkspaceCreateParams {
    WorkspaceCreateParams {
        project: project.to_owned(),
        name: non_empty(&details.name).unwrap_or_else(|| workspace_name_seed(prompt)),
        branch: non_empty(&details.branch),
        base: non_empty(&details.base),
        existing_branch: details.existing_branch,
        refresh_base: false,
        path: None,
        request_id: None,
        no_setup: false,
    }
}

/// The text trimmed, or `None` when nothing is left of it.
fn non_empty(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// The `agent.start` in the new workspace. A blank prompt starts the agent with none.
pub fn home_start_params(
    workspace: &str,
    kind: &str,
    prompt: &str,
    permission_mode: Option<String>,
    options: StartOptions,
) -> AgentStartParams {
    let mut params = start_params(workspace, kind, prompt, permission_mode, options);
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
        let params = create_params("p1", "fix the login", &WorkspaceDetails::default());
        assert_eq!(params.project, "p1");
        assert_eq!(params.name, "fix-the-login");
        assert_eq!(params.branch, None);
        assert_eq!(params.base, None);
        assert!(!params.existing_branch);
    }

    #[test]
    fn typed_details_name_the_workspace_and_add_its_branch_and_base() {
        let details = WorkspaceDetails {
            name: "  login-fix ".into(),
            branch: "feat/login".into(),
            base: "main".into(),
            existing_branch: false,
        };
        let params = create_params("p1", "fix the login", &details);
        assert_eq!(params.name, "login-fix");
        assert_eq!(params.branch.as_deref(), Some("feat/login"));
        assert_eq!(params.base.as_deref(), Some("main"));
        assert!(!params.existing_branch);
    }

    #[test]
    fn a_blank_name_with_no_prompt_words_asks_the_service_for_one() {
        let params = create_params("p1", "  ", &WorkspaceDetails::default());
        assert_eq!(params.name, "");
    }

    #[test]
    fn reusing_a_branch_needs_its_name_and_then_sends_it_as_existing() {
        let mut details = WorkspaceDetails {
            existing_branch: true,
            ..WorkspaceDetails::default()
        };
        assert_eq!(details.send_block(), Some(NAME_THE_BRANCH));
        details.branch = " feat/login ".into();
        assert_eq!(details.send_block(), None);
        let params = create_params("p1", "fix the login", &details);
        assert_eq!(params.branch.as_deref(), Some("feat/login"));
        assert!(params.existing_branch);
    }

    #[test]
    fn a_new_worktree_starts_from_the_base_the_menu_names_then_the_project_base() {
        let mut details = WorkspaceDetails::default();
        assert_eq!(branch_pill(&details, None).label, "New worktree");
        assert_eq!(
            branch_pill(&details, None).detail,
            "from the default branch"
        );
        assert_eq!(branch_pill(&details, Some("main")).detail, "from main");
        details.base = " develop ".into();
        assert_eq!(branch_pill(&details, Some("main")).detail, "from develop");
    }

    #[test]
    fn reusing_a_branch_shows_the_name_typed_or_asks_for_one() {
        let mut details = WorkspaceDetails {
            existing_branch: true,
            ..WorkspaceDetails::default()
        };
        assert_eq!(branch_pill(&details, Some("main")).label, "Existing branch");
        details.branch = " feat/login ".into();
        assert_eq!(
            branch_pill(&details, Some("main")),
            BranchPill {
                label: "feat/login".into(),
                detail: "existing branch".into(),
            }
        );
    }

    #[test]
    fn a_blank_prompt_starts_the_agent_with_no_prompt() {
        let none = StartOptions::default;
        assert_eq!(
            home_start_params("w9", "claude", "  ", None, none()).prompt,
            None
        );
        let params = home_start_params("w9", "claude", "fix it", Some("plan".into()), none());
        assert_eq!(params.workspace.as_deref(), Some("w9"));
        assert_eq!(params.prompt.as_deref(), Some("fix it"));
        assert_eq!(params.permission_mode.as_deref(), Some("plan"));
    }
}
