//! The start page with no workspace (D-AG-01, J3), where "+ Create" lands. Send creates a workspace
//! in the chosen project, named from the prompt, then starts the agent in it. Free of GPUI.

use massorca_protocol::methods::agents::AgentStartParams;
use massorca_protocol::methods::projects::Project;
use massorca_protocol::methods::workspaces::WorkspaceCreateParams;

use super::start_page::{StartOptions, start_params};
use super::workspace_flow::ProjectChoice;

/// The key of the page's composer among the workspace start composers; no workspace id is empty.
pub const HOME_START: &str = "";

/// Why a start-page control is off until the service can take it.
pub const NEEDS_NEWER_SERVICE: &str = "Needs a newer service.";

/// Why "No project" is off until the service can start an agent without a workspace (A95).
pub const NO_PROJECT_NEEDS_SERVICE: &str = NEEDS_NEWER_SERVICE;

/// Why the branch pill is off with "No project": the conversation runs in the conversations folder, with no worktree.
pub const NO_WORKTREE_REASON: &str =
    "A conversation with no project runs in the conversations folder, with no worktree.";

/// Why the "New worktree" switch stays on: the service makes a worktree for every Git project, and the main checkout is
/// not offered yet.
pub const NEW_WORKTREE_REASON: &str =
    "Every Git project gets its own worktree; working in the main checkout is not offered yet.";

/// Why the branch pill is off for a folder project: a folder is not a Git repository, so it has no worktree or branch.
pub const FOLDER_NO_GIT_REASON: &str =
    "This folder is not a Git repository, so it has no worktree or branch.";

/// Send's reason while there is no project to create the workspace in.
pub const ADD_A_PROJECT: &str = "Add a project to start a session.";

/// The project button's text while there is none.
pub const CHOOSE_A_PROJECT: &str = "Choose a project";

/// The longest name a prompt's words make: the name stops before the word that would pass it.
const SEED_MAX: usize = 40;

/// How many names a derived name is tried under when the service cannot make names unique: itself, then -2 to -9.
pub const FREE_NAME_TRIES: usize = 9;

/// Words a derived name never ends on: English and Spanish connectors and stop words.
const CONNECTORS: &[&str] = &[
    "a", "al", "an", "and", "as", "at", "be", "by", "con", "de", "del", "e", "el", "en", "for",
    "from", "in", "is", "it", "la", "las", "lo", "los", "o", "of", "on", "or", "para", "por",
    "que", "se", "su", "sus", "the", "to", "un", "una", "with", "y",
];

/// The plain ASCII letter a Latin letter with an accent reads as, so "qué" keeps its letters in a name.
fn plain_letter(c: char) -> char {
    match c {
        'á' | 'à' | 'ä' | 'â' | 'Á' | 'À' | 'Ä' | 'Â' => 'a',
        'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'Ó' | 'Ò' | 'Ö' | 'Ô' => 'o',
        'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
        'ñ' | 'Ñ' => 'n',
        'ç' | 'Ç' => 'c',
        other => other,
    }
}

/// A workspace name from the prompt's words: lowercase ASCII letters and digits joined by `-`, whole words only, within
/// `SEED_MAX` characters, and never ending on a connector. Empty when no word survives, which asks the service for a
/// generated name.
// Interim until the core session's generated-name option lands on `workspace.create`; its flag name comes later.
pub fn workspace_name_seed(prompt: &str) -> String {
    let folded: String = prompt.chars().map(plain_letter).collect();
    let words = folded
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase);
    let mut kept: Vec<String> = Vec::new();
    let mut length = 0;
    for word in words {
        let extra = usize::from(!kept.is_empty()) + word.len();
        if length + extra > SEED_MAX {
            break;
        }
        length += extra;
        kept.push(word);
    }
    while kept
        .last()
        .is_some_and(|word| CONNECTORS.contains(&word.as_str()))
    {
        kept.pop();
    }
    kept.join("-")
}

/// The name a derived name is tried under on attempt `attempt` (counted from 1): the name, then `-2` after it.
pub fn name_for_try(base: &str, attempt: usize) -> String {
    if attempt <= 1 {
        base.to_owned()
    } else {
        format!("{base}-{attempt}")
    }
}

/// Whether the start page derives the workspace's name from the prompt: a name the person typed is never changed.
pub fn name_is_derived(details: &WorkspaceDetails) -> bool {
    details.name.trim().is_empty()
}

/// The project the page creates in: the chosen one while the service lists it, else the first.
pub fn home_project<'a>(projects: &'a [Project], chosen: Option<&str>) -> Option<&'a Project> {
    chosen
        .and_then(|id| projects.iter().find(|project| project.id == id))
        .or_else(|| projects.first())
}

/// What Send starts in: "No project" when the person chose it, else the project `home_project` picks, or `None` when
/// there is none to create in.
pub fn home_choice(projects: &[Project], chosen: Option<&ProjectChoice>) -> Option<ProjectChoice> {
    if chosen == Some(&ProjectChoice::NoProject) {
        return Some(ProjectChoice::NoProject);
    }
    let id = match chosen {
        Some(ProjectChoice::Project(id)) => Some(id.as_str()),
        _ => None,
    };
    home_project(projects, id).map(|project| ProjectChoice::Project(project.id.clone()))
}

/// The branch popover's choices as Send reads them (D-AG-01). A blank base takes the project's base. The name is
/// derived from the prompt; a name typed anywhere is never changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceDetails {
    /// Blank takes the prompt's first words, or the service's generated name.
    pub name: String,
    /// The branch the new worktree starts from; blank takes the project's base.
    pub base: String,
}

/// The branch pill's two labels: the branch Send creates or reuses, and where a new worktree starts (D-AG-01).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchPill {
    pub label: String,
    pub detail: String,
}

/// The branch pill's labels: a new worktree from the base the popover names, else the project's base, else the default
/// branch.
pub fn branch_pill(details: &WorkspaceDetails, project_base: Option<&str>) -> BranchPill {
    let base = non_empty(&details.base)
        .or_else(|| project_base.and_then(non_empty))
        .unwrap_or_else(|| "the default branch".to_owned());
    BranchPill {
        label: "New worktree".to_owned(),
        detail: format!("from {base}"),
    }
}

/// The branch pill with "No project": Send starts the agent in the conversations folder, with no worktree.
pub fn no_project_branch_pill() -> BranchPill {
    BranchPill {
        label: "No worktree".to_owned(),
        detail: "conversations folder".to_owned(),
    }
}

/// The `workspace.create` that Send asks for: a new worktree in `project`, from the base the popover names when it
/// names one; without one the service starts from the project's base.
pub fn create_params(
    project: &str,
    prompt: &str,
    details: &WorkspaceDetails,
) -> WorkspaceCreateParams {
    WorkspaceCreateParams {
        project: project.to_owned(),
        name: non_empty(&details.name).unwrap_or_else(|| workspace_name_seed(prompt)),
        branch: None,
        base: non_empty(&details.base),
        existing_branch: false,
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
    grid: (u16, u16),
) -> AgentStartParams {
    start_in(
        Some(workspace),
        kind,
        prompt,
        permission_mode,
        options,
        grid,
    )
}

/// The `agent.start` with no workspace, which the service takes only with `agent-no-project/1`: the session runs in its
/// own folder in the conversations folder, with no worktree or branch.
pub fn no_project_start_params(
    kind: &str,
    prompt: &str,
    permission_mode: Option<String>,
    options: StartOptions,
    grid: (u16, u16),
) -> AgentStartParams {
    start_in(None, kind, prompt, permission_mode, options, grid)
}

/// The `agent.start` in `workspace`, or with none. A blank prompt starts the agent with none.
fn start_in(
    workspace: Option<&str>,
    kind: &str,
    prompt: &str,
    permission_mode: Option<String>,
    options: StartOptions,
    grid: (u16, u16),
) -> AgentStartParams {
    let mut params = start_params(workspace, kind, prompt, permission_mode, options, grid);
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
            "add-oauth2-google-github-more-providers"
        );
        assert_eq!(workspace_name_seed("¿Qué tal?"), "que-tal");
    }

    #[test]
    fn the_name_never_ends_on_a_connector_in_english_or_spanish() {
        assert_eq!(
            workspace_name_seed("Explora el proyecto actual y"),
            "explora-el-proyecto-actual"
        );
        assert_eq!(
            workspace_name_seed("Migrate the authentication service to the new identity provider"),
            "migrate-the-authentication-service"
        );
        assert_eq!(
            workspace_name_seed(
                "Implementar la paginación de resultados para el panel de administración"
            ),
            "implementar-la-paginacion-de-resultados"
        );
        assert_eq!(workspace_name_seed("Fix the bug in"), "fix-the-bug");
        assert_eq!(workspace_name_seed("the a and o y"), "");
    }

    #[test]
    fn the_name_keeps_whole_words_within_the_budget() {
        let prompt = "Migrate the authentication service to the new identity provider";
        let name = workspace_name_seed(prompt);
        assert!(name.len() <= SEED_MAX);
        let words: Vec<&str> = prompt.split(' ').collect();
        assert!(
            name.split('-')
                .all(|part| words.iter().any(|word| word.eq_ignore_ascii_case(part))),
            "every part of {name:?} is a whole word of the prompt"
        );
        assert_eq!(
            workspace_name_seed("extraordinarilylongmodulenameherereallyverylongindeed"),
            "",
            "a single word past the budget is not cut"
        );
    }

    #[test]
    fn a_retry_adds_the_suffix_from_the_second_try_on() {
        assert_eq!(name_for_try("fix-the-bug", 1), "fix-the-bug");
        assert_eq!(name_for_try("fix-the-bug", 2), "fix-the-bug-2");
        assert_eq!(
            name_for_try("fix-the-bug", FREE_NAME_TRIES),
            "fix-the-bug-9"
        );
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
        assert_eq!(name, "refactor-everything-inside");
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
        assert_eq!(
            params.base, None,
            "no base picked: the service starts from the project's base"
        );
        assert!(!params.existing_branch);
    }

    #[test]
    fn a_picked_base_is_the_base_the_worktree_starts_from() {
        let details = WorkspaceDetails {
            name: String::new(),
            base: " release ".into(),
        };
        let params = create_params("p1", "fix the login", &details);
        assert_eq!(params.base.as_deref(), Some("release"));
        assert_eq!(params.branch, None);
        assert!(!params.existing_branch);
    }

    #[test]
    fn a_typed_name_is_never_changed() {
        let details = WorkspaceDetails {
            name: "  login-fix ".into(),
            ..WorkspaceDetails::default()
        };
        assert_eq!(
            create_params("p1", "fix the login", &details).name,
            "login-fix"
        );
    }

    #[test]
    fn only_a_blank_name_is_derived_from_the_prompt() {
        let blank = WorkspaceDetails::default();
        let typed = WorkspaceDetails {
            name: "mine".into(),
            ..WorkspaceDetails::default()
        };
        assert!(name_is_derived(&blank));
        assert!(!name_is_derived(&typed), "a typed name is never changed");
    }

    #[test]
    fn a_blank_name_with_no_prompt_words_asks_the_service_for_one() {
        let params = create_params("p1", "  ", &WorkspaceDetails::default());
        assert_eq!(params.name, "");
    }

    #[test]
    fn a_new_worktree_starts_from_the_picked_base_then_the_project_base() {
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
    fn a_blank_prompt_starts_the_agent_with_no_prompt() {
        let none = StartOptions::default;
        assert_eq!(
            home_start_params("w9", "claude", "  ", None, none(), (120, 40)).prompt,
            None
        );
        let params = home_start_params(
            "w9",
            "claude",
            "fix it",
            Some("plan".into()),
            none(),
            (120, 40),
        );
        assert_eq!(params.workspace.as_deref(), Some("w9"));
        assert_eq!(params.prompt.as_deref(), Some("fix it"));
        assert_eq!(params.permission_mode.as_deref(), Some("plan"));
    }

    #[test]
    fn no_project_is_the_choice_when_chosen_and_otherwise_the_project_rule_stands() {
        let projects = [project("p1", "alpha"), project("p2", "beta")];
        assert_eq!(
            home_choice(&projects, Some(&ProjectChoice::NoProject)),
            Some(ProjectChoice::NoProject)
        );
        assert_eq!(
            home_choice(&projects, None),
            Some(ProjectChoice::Project("p1".to_owned()))
        );
        assert_eq!(home_choice(&[], None), None);
    }

    #[test]
    fn a_no_project_start_carries_the_picked_model_and_effort() {
        let options = StartOptions {
            model: Some("opus".to_owned()),
            effort: Some("xhigh".to_owned()),
        };
        let params = no_project_start_params("claude", "fix it", None, options, (120, 40));
        assert_eq!(params.workspace, None);
        assert_eq!(params.model.as_deref(), Some("opus"));
        assert_eq!(params.effort.as_deref(), Some("xhigh"));
    }

    #[test]
    fn a_no_project_start_carries_no_workspace_and_a_blank_prompt_sends_none() {
        let params = no_project_start_params(
            "claude",
            "fix it",
            Some("plan".into()),
            StartOptions::default(),
            (120, 40),
        );
        assert_eq!(params.workspace, None);
        assert_eq!(params.prompt.as_deref(), Some("fix it"));
        assert_eq!(params.permission_mode.as_deref(), Some("plan"));
        assert_eq!(
            no_project_start_params("claude", "  ", None, StartOptions::default(), (120, 40))
                .prompt,
            None
        );
    }
}
