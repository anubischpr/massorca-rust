//! The workspace start page as plain data (D-AG-01, J3): what the Canvas shows for a workspace with
//! no session open. The facts come from the records the service reported; nothing here starts a
//! session. Free of GPUI, so the rules test without a window.

use std::collections::BTreeMap;

use massorca_protocol::methods::agents::AgentStartParams;
use massorca_protocol::methods::workspaces::WorkspaceState;

use super::service_view::LOCAL_HOST;
use super::work_panel::{NEW_TERMINAL_COLS, NEW_TERMINAL_ROWS};
use crate::service::{ChatUnsupportedReason, KindChat, StoreState};

/// Why the page's actions that need git are off.
pub const GIT_LATER: &str = "Git operations come later.";
pub const VOICE_LATER: &str = "Voice input comes later.";
/// The Send button's reason while no agent is chosen, or the service is not connected.
pub const CHOOSE_AN_AGENT: &str = "Choose an agent to start a session.";
pub const NOT_CONNECTED: &str = "The service is not connected.";
/// The Send button's reason while no agent is installed and turned on.
pub const NO_AGENTS_INSTALLED: &str = "No agents installed.";

/// The composer's placeholder: the session composer's words (A33).
pub const COMPOSER_PLACEHOLDER: &str = "Ask anything, @ to mention files, / for commands";

/// Where a session started from the page opens: as a Conversation in the Canvas, or as the agent's
/// own terminal in the Work panel (D-AG-01, pv96). Chat is the default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StartMode {
    #[default]
    Chat,
    Terminal,
}

impl StartMode {
    /// The switch's segment for this mode.
    pub fn index(self) -> usize {
        match self {
            Self::Chat => 0,
            Self::Terminal => 1,
        }
    }

    /// The mode of the switch's segment `index`; anything past Chat is Terminal.
    pub fn from_index(index: usize) -> Self {
        if index == 0 {
            Self::Chat
        } else {
            Self::Terminal
        }
    }

    /// The mode a start opens in: this one while Chat is available, else Terminal.
    pub fn offered(self, chat_available: bool) -> Self {
        if chat_available { self } else { Self::Terminal }
    }
}

/// Why the Chat segment is off until the service offers chat sessions.
pub const CHAT_NEEDS_NEWER_SERVICE: &str = "Chat sessions need a newer service.";

/// Whether the Chat segment is on (decision 0101, D11): on while the service offers `agent-chat/1` and the picked
/// agent can chat. With no agent picked it stays on, and Send says what is missing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatAvailability {
    Available,
    /// Off, with the reason its tooltip gives.
    Unavailable(String),
}

impl ChatAvailability {
    /// `agent` is the picked agent's name and its chat support; `None` while no agent is picked.
    pub fn new(offered: bool, agent: Option<(&str, Option<&KindChat>)>) -> Self {
        if !offered {
            return Self::Unavailable(CHAT_NEEDS_NEWER_SERVICE.to_owned());
        }
        match agent {
            Some((_, Some(chat))) if chat.supported => Self::Available,
            Some((name, chat)) => Self::Unavailable(cannot_chat(name, chat)),
            None => Self::Available,
        }
    }

    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }

    /// The reason the segment is off, when it is.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Available => None,
            Self::Unavailable(reason) => Some(reason.as_str()),
        }
    }
}

/// Why the picked agent `name` cannot run a chat, as the service reported it (decision 0101, D10).
fn cannot_chat(name: &str, chat: Option<&KindChat>) -> String {
    match chat.and_then(|chat| chat.reason) {
        Some(ChatUnsupportedReason::NotDetected) => {
            format!("{name} was not found on this computer.")
        }
        Some(ChatUnsupportedReason::VersionUnknown) => {
            format!("{name}'s version could not be read.")
        }
        Some(ChatUnsupportedReason::VersionOutsideRange) => {
            match chat.and_then(version_and_range) {
                Some((version, range)) => {
                    format!("Chat needs {name} {range}; this computer has {version}.")
                }
                None => format!("{name} cannot run chat sessions."),
            }
        }
        None => format!("{name} cannot run chat sessions."),
    }
}

/// The version the service read and the range it supports, when it gave both.
fn version_and_range(chat: &KindChat) -> Option<(&str, &str)> {
    Some((chat.version.as_deref()?, chat.range.as_deref()?))
}

/// The note under the composer while Terminal is chosen (WS-40).
pub const TERMINAL_NOTE: &str =
    "The agent opens in a Work panel terminal. Some features are limited there.";

/// The key hints under the composer, each key with what it does (D-AG-01).
pub const KEY_HINTS: [(&str, &str); 4] = [
    ("Enter", "send"),
    ("Shift+Enter", "new line"),
    ("@", "mention files"),
    ("/", "commands"),
];

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

/// A session the service has not answered for yet: the workspace it starts in and the agent's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartingSession {
    pub workspace: String,
    pub kind_name: String,
}

impl StartingSession {
    /// The status under the composer, which Send's reason repeats while the start is in flight.
    pub fn status(&self) -> String {
        format!("Starting {} on {LOCAL_HOST}", self.kind_name)
    }
}

/// Why Send is off, or `None` when it is on: a start in flight, the service away, no agent installed, or no agent
/// chosen.
pub fn send_reason(
    connected: bool,
    agents_installed: bool,
    kind_chosen: bool,
    starting: Option<&StartingSession>,
) -> Option<String> {
    if let Some(starting) = starting {
        Some(starting.status())
    } else if !connected {
        Some(NOT_CONNECTED.to_owned())
    } else if !agents_installed {
        Some(NO_AGENTS_INSTALLED.to_owned())
    } else if !kind_chosen {
        Some(CHOOSE_AN_AGENT.to_owned())
    } else {
        None
    }
}

/// The model and effort a start asks for (capability `agent-options/1`, decision 0092). Both absent sends neither.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StartOptions {
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// The protocol value of a picker effort label: the values the Claude kind's `--effort` takes.
pub fn effort_value(label: &str) -> Option<&'static str> {
    match label {
        "Low" => Some("low"),
        "Medium" => Some("medium"),
        "High" => Some("high"),
        "Extra High" => Some("xhigh"),
        "Max" => Some("max"),
        _ => None,
    }
}

/// Whether the picker row `model` is a real model of `kind`, not the kind's own row. No kind declares models yet, so
/// each kind's only row is itself; the per-kind model lists from the kind options PR make the other rows real.
pub fn is_real_model(kind: &str, model: &str) -> bool {
    model != kind
}

/// The options a start sends for the picker's row `model` of `kind`, whose effort is the index `effort` of `efforts`
/// (the labels). Sent only when the service offers `agent-options/1`, the row is a real model, and the effort names a
/// level the model offers.
pub fn start_options(
    offered: bool,
    kind: &str,
    model: &str,
    efforts: &[&str],
    effort: Option<usize>,
) -> StartOptions {
    if !offered || !is_real_model(kind, model) {
        return StartOptions::default();
    }
    let effort = effort
        .and_then(|index| efforts.get(index))
        .and_then(|label| effort_value(label))
        .map(str::to_owned);
    StartOptions {
        model: Some(model.to_owned()),
        effort,
    }
}

/// The `agent.start` call that opens a session in `workspace` (none starts it with no project) with `prompt`, at the
/// Work panel's terminal size. A `permission_mode` is sent only when the person picked one; without it the vendor CLI
/// starts in its own default.
pub fn start_params(
    workspace: Option<&str>,
    kind: &str,
    prompt: &str,
    permission_mode: Option<String>,
    options: StartOptions,
) -> AgentStartParams {
    AgentStartParams {
        workspace: workspace.map(str::to_owned),
        kind: kind.to_owned(),
        prompt: Some(prompt.to_owned()),
        args: Vec::new(),
        env: BTreeMap::new(),
        read_only: false,
        title: None,
        cols: Some(NEW_TERMINAL_COLS),
        rows: Some(NEW_TERMINAL_ROWS),
        request_id: None,
        permission_mode,
        model: options.model,
        effort: options.effort,
    }
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

    #[test]
    fn the_composer_keeps_the_session_placeholder_and_its_key_hints() {
        assert_eq!(
            COMPOSER_PLACEHOLDER,
            "Ask anything, @ to mention files, / for commands"
        );
        let keys: Vec<&str> = KEY_HINTS.iter().map(|(key, _)| *key).collect();
        assert_eq!(keys, ["Enter", "Shift+Enter", "@", "/"]);
    }

    #[test]
    fn send_is_off_until_an_agent_is_chosen_and_the_service_is_connected() {
        assert_eq!(send_reason(true, true, true, None), None);
        assert_eq!(
            send_reason(true, true, false, None).as_deref(),
            Some(CHOOSE_AN_AGENT)
        );
        assert_eq!(
            send_reason(false, true, true, None).as_deref(),
            Some(NOT_CONNECTED)
        );
    }

    #[test]
    fn send_is_off_with_no_agent_installed_whatever_the_choice() {
        assert_eq!(
            send_reason(true, false, false, None).as_deref(),
            Some(NO_AGENTS_INSTALLED)
        );
        assert_eq!(
            send_reason(false, false, false, None).as_deref(),
            Some(NOT_CONNECTED)
        );
    }

    #[test]
    fn a_start_in_flight_says_which_agent_starts_where() {
        let starting = StartingSession {
            workspace: "w1".into(),
            kind_name: "Claude".into(),
        };
        assert_eq!(
            send_reason(true, true, true, Some(&starting)).as_deref(),
            Some("Starting Claude on This PC")
        );
    }

    #[test]
    fn chat_is_the_default_mode_and_each_segment_has_its_mode() {
        assert_eq!(StartMode::default(), StartMode::Chat);
        for mode in [StartMode::Chat, StartMode::Terminal] {
            assert_eq!(StartMode::from_index(mode.index()), mode);
        }
    }

    #[test]
    fn without_the_chat_capability_every_start_opens_on_terminal() {
        assert_eq!(StartMode::Chat.offered(false), StartMode::Terminal);
        assert_eq!(StartMode::Terminal.offered(false), StartMode::Terminal);
        assert_eq!(StartMode::Chat.offered(true), StartMode::Chat);
        assert_eq!(StartMode::Terminal.offered(true), StartMode::Terminal);
    }

    fn agent_chat(supported: bool, reason: Option<ChatUnsupportedReason>) -> KindChat {
        KindChat {
            supported,
            reason,
            version: None,
            range: None,
        }
    }

    #[test]
    fn chat_is_available_while_the_service_offers_it_and_the_picked_agent_can_chat() {
        let can = agent_chat(true, None);
        assert!(ChatAvailability::new(true, Some(("Claude", Some(&can)))).is_available());
        assert_eq!(
            ChatAvailability::new(false, Some(("Claude", Some(&can)))).reason(),
            Some(CHAT_NEEDS_NEWER_SERVICE)
        );
        assert!(
            ChatAvailability::new(true, None).is_available(),
            "with no agent picked Send says what is missing, and Chat stays on"
        );
    }

    #[test]
    fn each_reason_says_why_the_picked_agent_cannot_chat() {
        let reason_for = |chat: Option<KindChat>| {
            ChatAvailability::new(true, Some(("Claude", chat.as_ref())))
                .reason()
                .map(str::to_owned)
        };
        let not_detected = agent_chat(false, Some(ChatUnsupportedReason::NotDetected));
        assert_eq!(
            reason_for(Some(not_detected)).as_deref(),
            Some("Claude was not found on this computer.")
        );
        let unreadable = agent_chat(false, Some(ChatUnsupportedReason::VersionUnknown));
        assert_eq!(
            reason_for(Some(unreadable)).as_deref(),
            Some("Claude's version could not be read.")
        );
        assert_eq!(
            reason_for(Some(agent_chat(false, None))).as_deref(),
            Some("Claude cannot run chat sessions.")
        );
        assert_eq!(
            reason_for(None).as_deref(),
            Some("Claude cannot run chat sessions."),
            "a kind the service lists no chat for cannot chat"
        );
    }

    #[test]
    fn a_version_outside_the_range_names_the_range_and_the_version_found() {
        let outside = KindChat {
            supported: false,
            reason: Some(ChatUnsupportedReason::VersionOutsideRange),
            version: Some("2.4.1".to_owned()),
            range: Some(">=2.1.0, <2.3.0".to_owned()),
        };
        assert_eq!(
            ChatAvailability::new(true, Some(("Claude", Some(&outside)))).reason(),
            Some("Chat needs Claude >=2.1.0, <2.3.0; this computer has 2.4.1.")
        );
        let no_range = KindChat {
            range: None,
            ..outside
        };
        assert_eq!(
            ChatAvailability::new(true, Some(("Claude", Some(&no_range)))).reason(),
            Some("Claude cannot run chat sessions."),
            "without the range there is no sentence to say, so the plain reason stands"
        );
    }

    #[test]
    fn a_start_carries_the_workspace_the_kind_and_the_prompt() {
        let params = start_params(
            Some("w1"),
            "claude",
            "fix the login",
            None,
            StartOptions::default(),
        );
        assert_eq!(params.workspace.as_deref(), Some("w1"));
        assert_eq!(params.permission_mode, None);
        assert_eq!(params.kind, "claude");
        assert_eq!(params.prompt.as_deref(), Some("fix the login"));
        assert_eq!(params.cols, Some(NEW_TERMINAL_COLS));
    }

    /// The Claude kind's declared level ids, which a start sends as its `effort`.
    const LEVELS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

    #[test]
    fn the_declared_level_id_is_the_value_a_start_sends() {
        let codex = ["minimal", "low", "xhigh"];
        assert_eq!(
            start_options(true, "codex", "gpt-5", &codex, Some(0))
                .effort
                .as_deref(),
            Some("minimal"),
            "a kind's own level id is sent, whatever its label"
        );
        assert_eq!(
            start_options(true, "claude", "opus", &LEVELS, Some(3))
                .effort
                .as_deref(),
            Some("xhigh")
        );
    }

    #[test]
    fn without_the_capability_neither_model_nor_effort_is_sent() {
        let options = start_options(false, "claude", "opus", &LEVELS, Some(2));
        assert_eq!(options, StartOptions::default());
        let params = start_params(Some("w1"), "claude", "hi", None, options);
        assert_eq!((params.model, params.effort), (None, None));
    }

    #[test]
    fn a_kind_row_is_not_a_model_so_neither_is_sent_even_when_offered() {
        assert!(!is_real_model("claude", "claude"));
        assert_eq!(
            start_options(true, "claude", "claude", &[], None),
            StartOptions::default()
        );
    }

    #[test]
    fn a_real_model_sends_its_model_and_the_effort_the_row_chose() {
        let options = start_options(true, "claude", "opus", &LEVELS, Some(3));
        assert_eq!(
            options,
            StartOptions {
                model: Some("opus".to_owned()),
                effort: Some("xhigh".to_owned()),
            }
        );
    }

    #[test]
    fn a_model_without_an_effort_sends_the_model_alone_and_an_unknown_level_sends_no_effort() {
        assert_eq!(
            start_options(true, "claude", "haiku", &[], None).effort,
            None
        );
        assert_eq!(
            start_options(true, "claude", "opus", &["low"], Some(1)).effort,
            None
        );
        assert_eq!(
            start_options(true, "claude", "opus", &LEVELS, Some(9)).effort,
            None
        );
    }
}
