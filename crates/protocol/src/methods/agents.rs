//! Agent methods: coding agents run as programs in service-owned terminals (capability `agents/1`,
//! `docs/architecture/agents.md`, decisions 0050 to 0057).
//!
//! Every enum here has an `Unknown` variant for values from a newer peer; fields are only ever added (0002).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::methods::terminals::{TerminalExit, TerminalState};

/// Params: none. Result: [`AgentKinds`].
pub const KINDS: &str = "agent.kinds";
/// Params: [`AgentStartParams`]. Result: [`AgentSession`].
pub const START: &str = "agent.start";
/// Params: [`AgentListParams`]. Result: [`AgentSessionList`], newest first.
pub const LIST: &str = "agent.list";
/// Params: [`AgentSessionRef`]. Result: [`AgentSession`].
pub const SHOW: &str = "agent.show";
/// Params: [`AgentStopParams`]. Result: [`AgentSession`].
pub const STOP: &str = "agent.stop";
/// Params: [`AgentSessionRef`]. Result: [`AgentSession`], the forgotten record. The session must be exited.
pub const FORGET: &str = "agent.forget";
/// Params: [`AgentPromptParams`]. Result: [`PromptReceipt`].
pub const PROMPT: &str = "agent.prompt";
/// Params: [`AgentSessionRef`]. Result: [`AgentSession`].
pub const RESUME: &str = "agent.resume";
/// Params: [`AgentHookParams`]. Result: `null`.
pub const HOOK: &str = "agent.hook";
/// Params: [`AgentWatchParams`]. Result: `{ stream }`, as `events.subscribe`; then [`AgentWatchEvent`]s.
pub const WATCH: &str = "agent.watch";
/// Params: [`AgentDelegateParams`]. Result: [`AgentSession`], the child.
pub const DELEGATE: &str = "agent.delegate";
/// Params: [`AgentSessionRef`]. Result: [`AgentTree`].
pub const TREE: &str = "agent.tree";
/// Params: [`AgentBringBackParams`]. Result: [`BringBackResult`].
pub const BRING_BACK: &str = "agent.bring_back";
/// Params: [`CredentialsImportParams`]. Result: [`CredentialsImported`].
pub const CREDENTIALS_IMPORT: &str = "agent.credentials.import";
/// Params: [`CredentialsKindRef`]. Result: [`CredentialsShown`].
pub const CREDENTIALS_SHOW: &str = "agent.credentials.show";
/// Params: [`CredentialsKindRef`]. Result: [`CredentialsRemoved`].
pub const CREDENTIALS_REMOVE: &str = "agent.credentials.remove";

/// Most bytes of a prompt given at launch or through `agent.prompt` (0054).
pub const MAX_PROMPT_BYTES: usize = 8 * 1024;
/// Most bytes of a session's result text (0052).
pub const MAX_RESULT_BYTES: usize = 16 * 1024;
/// Most bytes of a hook payload (0053).
pub const MAX_HOOK_PAYLOAD_BYTES: usize = 64 * 1024;

/// How a kind's status reaches the service (0053).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StatusMechanism {
    ClaudeSettings,
    CodexNotify,
    EnvCommand,
    None,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Whether a kind can use a profile credential home, and whether the profile has one (0056).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindCredentials {
    /// The kind declares credential files, so `agent.credentials.import` works for it.
    pub supported: bool,
    /// The profile holds a credential home for the kind.
    pub imported: bool,
}

/// One declared agent kind, as `agent.kinds` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentKind {
    pub id: String,
    pub name: String,
    pub icon: String,
    /// `core` for built-ins, `user` for the profile's own files, `ext:<id>` later.
    pub owner: String,
    /// One of the program candidates resolves to a file; `agent.start` refuses kinds that are not installed.
    pub installed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program_path: Option<String>,
    /// The kind declares a resume form and a way to know the agent's own id.
    pub resumable: bool,
    pub status_mechanism: StatusMechanism,
    pub credentials: KindCredentials,
    /// Why the declaration is invalid. A kind with problems is listed but never started.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
}

/// Result of `agent.kinds`: built-ins first, then the profile's files by file name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentKinds {
    pub kinds: Vec<AgentKind>,
}

/// Params of `agent.start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentStartParams {
    /// Workspace id, name or path.
    pub workspace: String,
    pub kind: String,
    /// The first prompt, at most [`MAX_PROMPT_BYTES`] and without NUL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// Added after the kind's own launch arguments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Applied last to the run's environment.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// Declares that the session does not write; it is not counted by the one-writer rule.
    #[serde(default)]
    pub read_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cols: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<u16>,
    /// A retry with the same id returns the first session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// Params of `agent.list`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AgentListParams {
    /// Workspace id, name or path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
}

/// Result of `agent.list`, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSessionList {
    pub sessions: Vec<AgentSession>,
}

/// Selects one session by id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSessionRef {
    pub session: String,
}

/// Params of `agent.stop`. `force` skips the terminal's grace period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentStopParams {
    pub session: String,
    #[serde(default)]
    pub force: bool,
}

/// Whether the session's current or last run is there; the terminal vocabulary (0052).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Execution {
    pub state: TerminalState,
    /// How the last run ended, once it has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<TerminalExit>,
}

/// What the agent reports it is doing (0052).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityState {
    /// A turn is running.
    Working,
    /// The agent stopped to ask the person.
    Waiting,
    /// Ready; no turn has run since the run started.
    Idle,
    /// The last turn ended, with an outcome.
    Done,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// How a `done` turn ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Success,
    Failure,
    Cancelled,
    Interrupted,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Where an activity value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivitySource {
    /// The agent's hooks.
    Hook,
    /// Derived by the service from the run's exit.
    Process,
    /// Structured output; reserved.
    Output,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// The last activity an agent reported, with its source and the time the service recorded it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Activity {
    pub state: ActivityState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,
    pub source: ActivitySource,
    pub at_ms: i64,
}

/// One agent session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSession {
    /// UUID v7.
    pub id: String,
    pub kind: String,
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub read_only: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brought_back_at_ms: Option<i64>,
    /// The agent's own conversation id, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_ref: Option<String>,
    /// The current or last run's terminal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_id: Option<String>,
    pub created_at_ms: i64,
    pub execution: Execution,
    /// Absent until the agent reports; absent is not a state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<Activity>,
    /// The last turn's result text, at most [`MAX_RESULT_BYTES`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}
