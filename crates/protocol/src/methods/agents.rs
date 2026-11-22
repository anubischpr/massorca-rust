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
/// Params: [`AgentDelegateParams`]. Result: [`AgentDelegated`]: the child's [`AgentSession`] with an optional `note`.
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
#[serde(from = "AgentKindWire")]
pub struct AgentKind {
    pub id: String,
    pub name: String,
    pub icon: String,
    /// `core` for built-ins, `user` for the profile's own files, `ext:<id>` later.
    pub owner: String,
    /// Deprecated: the same value as `detected`, kept so `agents/1` clients keep working. Read `detected`
    /// and `package.state` instead; `agents/2` drops this field.
    pub installed: bool,
    /// One of the program candidates resolves to a file (0050). `agent.start` refuses kinds that are not detected.
    /// A peer that sends no `detected` (`agents/1`) gets the value of `installed`.
    pub detected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program_path: Option<String>,
    /// Whether the kind's package is available, enabled or disabled (0059).
    pub package: KindPackage,
    /// The kind declares a resume form and a way to know the agent's own id.
    pub resumable: bool,
    pub status_mechanism: StatusMechanism,
    pub credentials: KindCredentials,
    /// Why the declaration is invalid. A kind with problems is listed but never started.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub problems: Vec<String>,
}

/// The decoded form of [`AgentKind`]. Only the fallback for an absent `detected` lives here: an `agents/1` peer
/// sends `installed` alone, and there it already meant "the program resolves" (0059).
#[derive(Deserialize)]
struct AgentKindWire {
    id: String,
    name: String,
    icon: String,
    owner: String,
    installed: bool,
    #[serde(default)]
    detected: Option<bool>,
    #[serde(default)]
    program_path: Option<String>,
    #[serde(default)]
    package: KindPackage,
    resumable: bool,
    status_mechanism: StatusMechanism,
    credentials: KindCredentials,
    #[serde(default)]
    problems: Vec<String>,
}

impl From<AgentKindWire> for AgentKind {
    fn from(wire: AgentKindWire) -> Self {
        Self {
            detected: wire.detected.unwrap_or(wire.installed),
            id: wire.id,
            name: wire.name,
            icon: wire.icon,
            owner: wire.owner,
            installed: wire.installed,
            program_path: wire.program_path,
            package: wire.package,
            resumable: wire.resumable,
            status_mechanism: wire.status_mechanism,
            credentials: wire.credentials,
            problems: wire.problems,
        }
    }
}

/// The package a kind comes from (0059). Built-ins and profile files are `enabled` with no id until packages land.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindPackage {
    #[serde(default)]
    pub state: PackageState,
    /// The catalogue id of the package, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The installed version of the package, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// The state of a kind's package (0059).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PackageState {
    /// Listed in the catalogue, not installed.
    Available,
    /// Installed and on.
    Enabled,
    /// Installed and off.
    Disabled,
    /// A value from a newer peer that this build does not know. Also what a kind from an `agents/1` peer decodes to.
    #[default]
    #[serde(other)]
    Unknown,
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

/// Params of `agent.prompt`. `force` types even while the agent is working or waiting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPromptParams {
    pub session: String,
    pub text: String,
    #[serde(default)]
    pub force: bool,
}

/// What `agent.prompt` can honestly say about a delivery (0054).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptKind {
    /// The bytes were written to the terminal; nothing more is known.
    HandedToTerminal,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Result of `agent.prompt`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptReceipt {
    pub receipt: ReceiptKind,
    pub bytes: u64,
}

/// Params of `agent.hook`, sent by `massorca agent hook` from inside a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentHookParams {
    pub session: String,
    pub token: String,
    pub event: String,
    /// The hook's payload, kept opaque; at most [`MAX_HOOK_PAYLOAD_BYTES`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
}

/// Params of `agent.watch`. Without a filter every session is watched.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AgentWatchParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

/// One item of an `agent.watch` stream, tagged by `kind`. Readers ignore kinds they do not know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentWatchEvent {
    /// The first item, and the item after a watcher fell behind: every selected session.
    Snapshot { sessions: Vec<AgentSession> },
    /// One session changed; the full record follows.
    Changed { session: Box<AgentSession> },
    /// A kind this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Params of `agent.delegate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDelegateParams {
    pub parent: String,
    pub prompt: String,
    /// Defaults to the parent's kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Name of the child workspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub read_only: bool,
    /// The parent run's token, required when called from inside that run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// Result of `agent.delegate`: the child's record, with its fields at the top level, and an optional note. A reader that
/// expects an [`AgentSession`] reads it unchanged; `note` was added for the result to say that the parent's uncommitted
/// changes were not carried over (0055).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDelegated {
    #[serde(flatten)]
    pub session: AgentSession,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Result of `agent.tree`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentTree {
    pub root: AgentSession,
    pub children: Vec<AgentSession>,
}

fn yes() -> bool {
    true
}

/// Params of `agent.bring_back`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentBringBackParams {
    pub child: String,
    /// Merge the child's branch into the parent's worktree.
    #[serde(default = "yes")]
    pub merge: bool,
    /// Tell the parent run about the child.
    #[serde(default = "yes")]
    pub prompt: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

/// Result of `agent.bring_back`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BringBackResult {
    pub child: AgentSession,
    pub merged: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_commit: Option<String>,
    pub prompt_delivered: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Params of `agent.credentials.import`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialsImportParams {
    pub kind: String,
    /// The directory the declared files are copied from.
    pub from: String,
    #[serde(default)]
    pub replace: bool,
}

/// One copied credential file: its name and size, never its contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedFile {
    pub name: String,
    pub bytes: u64,
}

/// Result of `agent.credentials.import`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialsImported {
    pub kind: String,
    pub home: String,
    pub files: Vec<ImportedFile>,
}

/// Selects one kind's credential home.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialsKindRef {
    pub kind: String,
}

/// One declared credential file in a profile home.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialFile {
    pub name: String,
    pub present: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at_ms: Option<i64>,
}

/// Result of `agent.credentials.show`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialsShown {
    pub kind: String,
    pub home: String,
    pub exists: bool,
    pub files: Vec<CredentialFile>,
}

/// Result of `agent.credentials.remove`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialsRemoved {
    pub kind: String,
    pub removed: bool,
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
