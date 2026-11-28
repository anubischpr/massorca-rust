//! Agent methods: coding agents run as programs in service-owned terminals (capability `agents/1`,
//! `docs/architecture/agents.md`, decisions 0050 to 0057).
//!
//! Every enum here has an `Unknown` variant for values from a newer peer; fields are only ever added (0002).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::methods::chat::{KindChat, RunType};
use crate::methods::terminals::{TerminalExit, TerminalState};
use crate::methods::workspaces::{WorkspaceKind, WorkspaceRemoved};

/// Params: none. Result: [`AgentKinds`].
pub const KINDS: &str = "agent.kinds";
/// Params: [`AgentModelsParams`]. Result: [`AgentModels`] (capability `agent-models/1`, decision 0166).
pub const MODELS: &str = "agent.models";
/// Params: [`AgentStartParams`]. Result: [`AgentSession`].
pub const START: &str = "agent.start";
/// Params: [`AgentListParams`]. Result: [`AgentSessionList`], newest first.
pub const LIST: &str = "agent.list";
/// Params: [`AgentSessionRef`]. Result: [`AgentSession`].
pub const SHOW: &str = "agent.show";
/// Params: [`AgentStopParams`]. Result: [`AgentSession`].
pub const STOP: &str = "agent.stop";
/// Params: [`AgentForgetParams`] (an [`AgentSessionRef`] with optional `remove_workspace` and `force`). Result:
/// [`AgentForgotten`], the forgotten record with what became of its workspace. The session must be exited.
pub const FORGET: &str = "agent.forget";
/// Params: [`AgentSessionRef`]. Result: [`AgentForgetPreview`]: what `agent.forget` with `remove_workspace` would meet,
/// read without changing anything. Capability `agent-archive/1` (decision 0163).
pub const FORGET_PREVIEW: &str = "agent.forget_preview";
/// Params: [`AgentArchiveSessionParams`]. Result: [`AgentSession`]. Archives (hides) an exited session, or clears that;
/// a live run is refused with `agent.busy`. Capability `agent-archive/1` (decision 0163).
pub const ARCHIVE_SESSION: &str = "agent.archive_session";
/// Params: [`AgentRenameParams`]. Result: [`AgentSession`]. Sets a live or exited session's title, trimmed: 1 to
/// [`MAX_TITLE_CHARS`] characters without control characters, else `request.invalid_params`. Capability
/// `agent-archive/1` (decision 0163).
pub const RENAME: &str = "agent.rename";
/// Longest title `agent.rename` takes, in characters after trimming.
pub const MAX_TITLE_CHARS: usize = 120;
/// Params: [`AgentSessionRef`]. Result: [`AgentSession`], released: its terminal output is archived and its terminal freed,
/// and the record stays. The session must be exited; a live run is refused with `agent.busy`. Capability `agent-release/1`
/// (decision 0090).
pub const RELEASE: &str = "agent.release";
/// Params: [`AgentRetainParams`]. Result: [`AgentSession`]. Sets or clears the durable retain of a session (capability
/// `agent-release/1`, decision 0090).
pub const RETAIN: &str = "agent.retain";
/// Params: [`AgentSessionRef`]. Result: [`AgentArchive`], the archived output of a released session (capability
/// `agent-release/1`, decision 0090).
pub const ARCHIVE: &str = "agent.archive";
/// Params: [`AgentPromptParams`]. Result: [`PromptReceipt`].
pub const PROMPT: &str = "agent.prompt";
/// Params: [`AgentResumeParams`] (an [`AgentSessionRef`] with an optional mode). Result: [`AgentSession`].
pub const RESUME: &str = "agent.resume";
/// Params: [`AgentForkParams`]. Result: [`AgentSession`], the new session. Capability `agent-fork/1` (decision 0168).
pub const FORK: &str = "agent.fork";
/// Params: [`AgentSetModeParams`]. Result: [`AgentModeSet`]. Capability `agent-modes/1` (decision 0083).
pub const SET_MODE: &str = "agent.set_mode";
/// Changes a session's model, effort or fast mode (decision 0171). Capability `agent-options-live/1`.
pub const SET_OPTIONS: &str = "agent.set_options";
/// Params: [`AgentMarkSeenParams`]. Result: [`AgentSession`]. Marks a session's latest `waiting` or `done` as seen
/// (capability `agent-seen/1`).
pub const MARK_SEEN: &str = "agent.mark_seen";
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

pub const ACCOUNTS_LIST: &str = "agent.accounts.list";
pub const ACCOUNTS_ADD: &str = "agent.accounts.add";
/// Params: [`AccountsSignInParams`]. Result: [`AccountsAdded`]. Signs an existing account in again (decision 0059).
pub const ACCOUNTS_SIGN_IN: &str = "agent.accounts.sign_in";
pub const ACCOUNTS_IMPORT: &str = "agent.accounts.import";
/// Params: [`ImportCandidatesParams`]. Result: [`ImportCandidates`]. Capability `agent-import-detect/1` (decision 0097).
pub const ACCOUNTS_IMPORT_CANDIDATES: &str = "agent.accounts.import_candidates";
pub const ACCOUNTS_ACTIVATE: &str = "agent.accounts.activate";
pub const ACCOUNTS_REMOVE: &str = "agent.accounts.remove";
pub const ACCOUNTS_CHECK: &str = "agent.accounts.check";
/// Names a chat from its first prompt and reply by a one-shot run of its kind (decision 0167, capability `agent-title/1`).
pub const GENERATE_TITLE: &str = "agent.generate_title";

/// Params: [`AgentQueueParams`]. Result: [`QueuedPrompt`]. Capability `agent-queue/1` (decision 0084).
pub const QUEUE: &str = "agent.queue";
/// Params: [`AgentSessionRef`]. Result: [`AgentQueueList`]. Capability `agent-queue/1` (decision 0084).
pub const QUEUE_LIST: &str = "agent.queue.list";
/// Params: [`AgentQueueEditParams`]. Result: [`QueuedPrompt`]. Capability `agent-queue/1` (decision 0084).
pub const QUEUE_EDIT: &str = "agent.queue.edit";
/// Params: [`AgentQueueItemRef`]. Result: [`QueuedPrompt`]. Capability `agent-queue/1` (decision 0084).
pub const QUEUE_REMOVE: &str = "agent.queue.remove";
/// Params: [`AgentQueueItemRef`]. Result: [`QueuedPrompt`]. Capability `agent-queue/1` (decision 0084).
pub const QUEUE_SEND_NOW: &str = "agent.queue.send_now";
/// Params: [`AgentSessionRef`]. Result: [`AgentQueueList`]. Capability `agent-queue/1` (decision 0084).
pub const QUEUE_RESUME: &str = "agent.queue.resume";

/// Most prompts one session's queue holds (decision 0084).
pub const MAX_QUEUED_PROMPTS: usize = 100;

/// Most request ids of delivered prompts one session remembers, so a retry of a delivered prompt is refused (decision
/// 0084). The window is the most recent ids, not a span of time.
pub const MAX_DELIVERED_REQUEST_IDS: usize = 100;

/// `reason` of a waiting prompt held because its agent is busy.
pub const QUEUE_REASON_BUSY: &str = "busy";
/// `reason` of a waiting prompt held because its agent is asking the person something.
pub const QUEUE_REASON_ASKING: &str = "asking";
/// `reason` of a waiting prompt paused by the person's stop.
pub const QUEUE_REASON_PAUSED_STOPPED: &str = "paused_stopped";
/// `reason` of a waiting prompt paused by a service restart.
pub const QUEUE_REASON_PAUSED_RESTARTED: &str = "paused_restarted";
/// `reason` of a waiting prompt held behind a failed delivery.
pub const QUEUE_REASON_BEHIND_FAILED: &str = "behind_failed";
/// `reason` of a waiting prompt held because a writing child of the session works in the session's own workspace: the
/// parent waits until that child finishes (decision 0170).
pub const QUEUE_REASON_CHILD_WRITING: &str = "child_writing";

/// Params of `agent.queue`. `request_id`, when given, makes a repeated call return the prompt it queued first. The
/// attachments are checked when the prompt is queued and again when it is typed (decision 0172).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentQueueParams {
    pub session: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<PromptAttachment>,
}

/// Params of `agent.queue.edit`. Only a waiting or failed prompt can be edited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentQueueEditParams {
    pub id: String,
    pub text: String,
}

/// Params of `agent.queue.remove` and `agent.queue.send_now`: a queued prompt's id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentQueueItemRef {
    pub id: String,
}

/// Where a queued prompt is in its delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueState {
    /// Not delivered yet; `reason` says what holds it.
    Waiting,
    /// Its text is being typed into the run.
    Sending,
    /// Its delivery failed; `reason` says why. Nothing behind it is delivered until it is removed or sent.
    Failed,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// One prompt waiting in a session's queue (decision 0084).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedPrompt {
    pub id: String,
    pub session: String,
    /// Its place in the queue, from 1; the lowest is delivered first.
    pub position: u64,
    pub text: String,
    pub state: QueueState,
    /// For a waiting prompt, one of the `QUEUE_REASON_*` values; for a failed prompt, why its delivery failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// Prints every field but the text, which is replaced by its length: a queued prompt is never printed into a log.
impl std::fmt::Debug for QueuedPrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueuedPrompt")
            .field("id", &self.id)
            .field("session", &self.session)
            .field("position", &self.position)
            .field("text", &format_args!("{} bytes", self.text.len()))
            .field("state", &self.state)
            .field("reason", &self.reason)
            .field("created_at_ms", &self.created_at_ms)
            .field("updated_at_ms", &self.updated_at_ms)
            .finish()
    }
}

/// Result of `agent.queue.list` and `agent.queue.resume`: a session's queue, head first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentQueueList {
    pub session: String,
    pub items: Vec<QueuedPrompt>,
}

/// Most bytes of a prompt given at launch or through `agent.prompt` (0054).
pub const MAX_PROMPT_BYTES: usize = 8 * 1024;
/// Most characters of a session's `preview` (decision 0168).
pub const MAX_PREVIEW_CHARS: usize = 200;
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
    /// `core` for first-party (`massorca.*`) packages, `user` for the profile's own files, `ext:<package id>` for other
    /// packages.
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
    /// The permission modes the kind declares (decision 0083), in declared order. Absent from an older peer: none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub permission_modes: Vec<PermissionMode>,
    /// The id of the mode the vendor CLI starts in by itself, for display only (decision 0083). It is informational: the
    /// vendor's real default may differ by CLI version, so it must never be shown as a session's mode. A session in the
    /// vendor's default has `mode_source: vendor_default` and no `permission_mode`; label it "the CLI's default".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_permission_mode: Option<String>,
    /// Whether the kind can run as a structured chat (capability `agent-chat/1`, decision 0101), and why not. Absent from
    /// a kind without chat support and from an older peer: no chat.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat: Option<KindChat>,
    /// The `model` the kind takes at start, with its choices (decisions 0094, 0096). Absent when the kind takes no model,
    /// and from a peer that predates it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<KindModel>,
    /// The `effort` the kind takes at start, with its levels (decisions 0094, 0096). Absent when the kind takes no
    /// effort, and from a peer that predates it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<KindEffort>,
    /// Fast mode the kind takes at start (decision 0165). Absent from a peer that predates it; a kind that declares no fast
    /// mode says so with `supported: false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<KindFast>,
}

/// The `fast` option of a kind (decision 0165), as `agent.kinds` lists it: whether it takes fast mode at start, and why not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindFast {
    pub supported: bool,
    /// Why the kind takes no fast mode, when `supported` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The fast mode of a session (decision 0165): whether its current or last run was started with it on, and whether
/// something confirmed the vendor took it. Nothing confirms it yet, so `confirmed` is false.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionFast {
    pub on: bool,
    pub confirmed: bool,
}

/// The `model` option of a kind (decision 0094), as `agent.kinds` lists it: the choices its picker shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindModel {
    /// The choices in declared order. Empty when the list is account-scoped and the vendor CLI reports it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<KindModelChoice>,
}

/// One model choice of a kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindModelChoice {
    pub id: String,
    pub label: String,
    /// The model the picker preselects.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub default: bool,
    /// The effort levels this model accepts, by id. Absent: every level of the kind. Empty: the model takes no effort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efforts: Option<Vec<String>>,
    /// Whether the model takes fast mode (decision 0165). Omitted when it does not.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fast: bool,
}

/// The `effort` option of a kind (decision 0094), as `agent.kinds` lists it: the levels its picker shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindEffort {
    /// The levels in declared order. Empty: the kind declares none, so any value a start names is passed on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levels: Vec<KindEffortLevel>,
    /// The level the picker preselects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// One effort level of a kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindEffortLevel {
    pub id: String,
    pub label: String,
}

/// One permission mode of a kind, as `agent.kinds` lists it (decision 0083). How the mode is passed stays in the
/// declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionMode {
    /// The vendor's own id of the mode, as `agent.start` and `agent.set_mode` take it.
    pub id: String,
    /// The vendor's name for the mode.
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The mode lifts the vendor's permission prompts or sandbox. It is never the default and must always be named.
    #[serde(default)]
    pub bypass: bool,
    /// A running session can switch to the mode at once (`agent.set_mode` replies `live`); otherwise a switch applies at
    /// the next resume.
    #[serde(default)]
    pub live_switch: bool,
    /// The vendor may start a run asked for this mode in another one without failing, so it is never confirmed.
    #[serde(default)]
    pub may_fall_back: bool,
    /// A lucide icon name for the mode (decision 0163). Absent when the kind names none, and from an older peer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// How a picker presents the mode (decision 0163); every bypass mode is `warning`. Absent when the kind names none,
    /// and from an older peer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<ModeTone>,
    /// Whether a picker offers the mode (decision 0163). An unoffered mode is still recognised, so a session reporting it
    /// shows its name. Sent only when false; absent (an older peer too) means offered.
    #[serde(default = "mode_offered", skip_serializing_if = "is_mode_offered")]
    pub selectable: bool,
}

fn mode_offered() -> bool {
    true
}

fn is_mode_offered(selectable: &bool) -> bool {
    *selectable
}

/// How a picker presents a permission mode (decision 0163).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeTone {
    /// An ordinary mode.
    Normal,
    /// A mode that lifts the vendor's prompts or sandbox.
    Warning,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
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
    #[serde(default)]
    permission_modes: Vec<PermissionMode>,
    #[serde(default)]
    default_permission_mode: Option<String>,
    #[serde(default)]
    chat: Option<KindChat>,
    model: Option<KindModel>,
    #[serde(default)]
    effort: Option<KindEffort>,
    #[serde(default)]
    fast: Option<KindFast>,
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
            permission_modes: wire.permission_modes,
            default_permission_mode: wire.default_permission_mode,
            chat: wire.chat,
            model: wire.model,
            effort: wire.effort,
            fast: wire.fast,
        }
    }
}

/// The package a kind comes from (0059). A profile file has no package: it is `enabled` with no id or version.
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
    /// Workspace id, name or path. Absent starts a session with no project (capability `agent-no-project/1`, decision
    /// 0094): it runs in the conversations folder, in a folder of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
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
    /// One of the kind's permission modes (decision 0083, capability `agent-modes/1`); the kind's default when absent.
    /// An id the kind does not declare is refused with `agent.mode_unknown`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    /// The model the kind's `model` flag takes (capability `agent-options/1`, decision 0094). A kind that declares no
    /// model flag refuses it with `agent.option_unsupported`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The reasoning effort the kind's `effort` flag takes (capability `agent-options/1`, decision 0094). A kind that
    /// declares no effort flag refuses it with `agent.option_unsupported`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// Fast mode for this start (capability `agent-fast/1`, decision 0165): passed as the kind's `fast` settings. A kind
    /// that declares no fast mode, or a model that does not take it, refuses it with `agent.option_unsupported`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<bool>,
}

/// Params of `agent.list`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AgentListParams {
    /// Workspace id, name or path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// List archived sessions too (capability `agent-archive/1`, decision 0163); they are left out without it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub include_archived: bool,
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

/// Params of `agent.mark_seen`. `at_ms` is the `at_ms` of the activity the caller showed; with it, a newer activity is
/// left unseen. Without it (an older caller), the current `waiting` or `done` is marked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMarkSeenParams {
    pub session: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_ms: Option<i64>,
}

/// Params of `agent.stop`. `force` skips the terminal's grace period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentStopParams {
    pub session: String,
    #[serde(default)]
    pub force: bool,
}

/// A file of the session's workspace sent with a prompt or a queued prompt (decision 0172). `path` is relative to the
/// workspace root; an absolute path is accepted only when it names a file inside the root. The service refuses the prompt
/// with `agent.attachment_invalid` for a path it cannot accept, and with `agent.attachment_unsupported` for a file the run
/// cannot take.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptAttachment {
    pub path: String,
}

/// Params of `agent.prompt`. `force` types even while the agent is working or waiting. `attachments` needs capability
/// `agent-prompt-attachments/1` (decision 0172).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPromptParams {
    pub session: String,
    pub text: String,
    #[serde(default)]
    pub force: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<PromptAttachment>,
}

/// What `agent.prompt` can honestly say about a delivery (0054).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptKind {
    /// The bytes were written to the terminal; nothing more is known.
    HandedToTerminal,
    /// The text was written to a chat run's structured input as a user message (decision 0101); the transcript shows
    /// it as a `user_message` item. Nothing more is known.
    SentToChat,
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

/// Params of `agent.resume`. Without `permission_mode`, the run takes the mode `agent.set_mode` stored for it, else the
/// session's last mode unless that is a bypass mode, else the kind's default (decision 0083).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentResumeParams {
    pub session: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    /// The terminal's columns, for the view that shows it; the default when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cols: Option<u16>,
    /// The terminal's rows, for the view that shows it; the default when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<u16>,
}

/// Params of `agent.fork` (decision 0168): a new session that continues a copy of `session`'s conversation, in the same
/// workspace, with the kind's fork arguments. The source keeps its own conversation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentForkParams {
    /// The source session: id, or a unique prefix of at least 8 characters. Its run must have exited.
    pub session: String,
    /// The new run's first prompt, at most [`MAX_PROMPT_BYTES`], delivered as a first start delivers one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// One of the kind's permission modes; the kind's default when absent, as a start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    /// The new session's title; the source's title when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Params of `agent.set_mode`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSetModeParams {
    pub session: String,
    pub permission_mode: String,
    /// Types a live switch into a run whose kind reports no activity, where MassOrca cannot tell whether the agent is in
    /// the middle of a prompt; refused with `agent.mode_unavailable` without it.
    #[serde(default)]
    pub force: bool,
}

/// When a mode set with `agent.set_mode` takes effect (decision 0083).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeApplied {
    /// The kind's live switch was typed into the run's terminal. Nothing confirms the agent took it, so the session's
    /// `mode_confirmed` is false.
    Live,
    /// The mode is stored and the next `agent.resume` launches in it.
    NextResume,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Result of `agent.set_mode`: when the mode applies, and the session after the change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentModeSet {
    pub applied: ModeApplied,
    pub session: AgentSession,
}

/// Params of `agent.set_options` (decision 0171). An option left out keeps its value; at least one must be named. The
/// values follow `agent.start`: a `model` and an `effort` the kind declares, and `fast` on a model that takes it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSetOptionsParams {
    pub session: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<bool>,
}

/// Where one option that `agent.set_options` changed took effect (decision 0171).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionApplied {
    /// The option was typed into the running agent, through the live command its kind declares. Nothing confirms the
    /// agent took it: a fast mode the agent did not take stays unconfirmed (the session's `fast.confirmed`).
    Live,
    /// The option is kept for the session's next `agent.resume`; the running agent keeps its value until then.
    NextResume,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Where each option that `agent.set_options` changed took effect (decision 0171). An option that already had the value
/// asked for is absent: nothing is typed for it, though its value is still kept for the next resume.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionsApplied {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<OptionApplied>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<OptionApplied>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<OptionApplied>,
}

/// Result of `agent.set_options`: where each changed option took effect, and the session after the change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentOptionsSet {
    pub applied: OptionsApplied,
    pub session: AgentSession,
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
    /// The child session's title; the first line of `prompt` by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub read_only: bool,
    /// Run a writing child in the parent's own workspace instead of a new worktree (decision 0170): the person's manual
    /// pick. The parent then waits until the child finishes. Absent means false; a peer older than 0170 ignores it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub same_workspace: bool,
    /// The parent run's token, required when called from inside that run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// The child's permission mode (decision 0083); the child kind's default when absent, never the parent's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
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

fn is_false(value: &bool) -> bool {
    !*value
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
    /// Why a state file was built without its source (decision 0099), for example `the source is not a JSON object`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    /// When someone marked this `waiting` or `done` as seen (capability `agent-seen/1`). Absent means not seen, and is
    /// what an older service sends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seen_at_ms: Option<i64>,
}

/// One agent session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    /// The current or last run's terminal. A chat session has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_id: Option<String>,
    /// How the session's run talks to the service (decision 0101). Absent means `terminal`, as from an older peer.
    #[serde(default, skip_serializing_if = "RunType::is_terminal")]
    pub run: RunType,
    pub created_at_ms: i64,
    pub execution: Execution,
    /// Absent until the agent reports; absent is not a state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<Activity>,
    /// The last turn's result text, at most [`MAX_RESULT_BYTES`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// The current or last run received vault secrets, or descends from one that did (decision 0071): its terminal,
    /// prompts, resume and delegation need host-admin scope. Never their names.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub holds_secrets: bool,
    /// Set in the reply to a start, resume or delegation whose run got no vault variables, saying why.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secrets_withheld: Option<String>,
    /// The permission mode the session runs in, or was last asked to (decision 0083). Absent when its kind declares no
    /// modes, and from a peer without capability `agent-modes/1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    /// The mode was passed as launch arguments of the current run, no caller arguments followed it, and its vendor never
    /// falls back from it. False (absent) otherwise: a typed live switch no hook confirmed, caller arguments that may
    /// override it, a mode marked `may_fall_back`, or the vendor's own default.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mode_confirmed: bool,
    /// Where the current or last run's mode came from (decision 0083): `vendor_default` (no flag was passed, and
    /// `permission_mode` is absent) or `chosen`. Absent when the kind declares no modes, and from an older peer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_source: Option<ModeSource>,
    /// A mode `agent.set_mode` stored for the next resume; the running mode is unchanged until then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_permission_mode: Option<String>,
    /// Whether the session is retained (decision 0090): its exited terminal is kept from eviction until a release or a
    /// `agent.retain` clears it. Absent when false, and from a peer without capability `agent-release/1`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retained: bool,
    /// Whether the session was released (decision 0090): its terminal output is archived, its terminal is freed and its
    /// record stays. Absent when false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub released: bool,
    /// Whether the session is archived (hidden, decision 0163): `agent.list` leaves it out unless asked, and nothing it
    /// needs is cleaned up. Absent when false, and from a peer without capability `agent-archive/1`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub archived: bool,
    /// Whether `agent.resume` could start a new run of the session now (decision 0160). Set by the service on every
    /// session view; absent from an older peer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<ResumeState>,
    /// What children the session could delegate now (decision 0161). Set by the service on every session view; absent
    /// from an older peer. Boxed to keep the record small.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delegation: Option<Box<DelegationOptions>>,
    /// Notes on the session in plain words (decision 0161): that a writing child of a plain folder shares its parent's
    /// folder, and that the current run got no delegation instructions because its CLI does not take them. Empty
    /// (absent) otherwise, and from an older peer. The current run's notes also say what a resume or fork left out of the
    /// stored options, and that a first input the agent never took (decision 0168).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Whether the run was started with fast mode on (capability `agent-fast/1`, decision 0165). Absent when off, and
    /// from a peer without the capability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast: Option<SessionFast>,
    /// The other live writing sessions in the folder workspace this live session writes in (decision 0169). Empty
    /// (absent) when there are none, for a worktree, for a read-only or exited session, and from an older peer. A client
    /// shows that another session also writes in the folder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shared_folder_with: Vec<String>,
    /// Start options, lineage, branch and preview (decision 0168), flat on the wire.
    #[serde(flatten)]
    pub meta: SessionMeta,
}

/// What a session view shows beside its record for a list (decision 0168). Every field is optional and flat on the
/// wire of [`AgentSession`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMeta {
    /// The model the session's latest run used (decision 0168): the one it was started with, kept across resumes and
    /// forks while the kind still takes it. Absent when none was named, or the kind no longer takes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The reasoning effort the session was started with (decision 0168), kept across resumes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// The branch of the session's workspace now (decision 0168). Absent for a plain folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// A short text for a session list (decision 0168), at most [`MAX_PREVIEW_CHARS`] characters: a chat's newest user or
    /// assistant message, or a terminal session's `result`. Withheld as `result` is from a run holding secrets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    /// The session this one was forked from with `agent.fork` (decision 0168).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forked_from: Option<String>,
}

/// The children a session could delegate now (decision 0161).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegationOptions {
    pub writing_child: WritingChild,
    /// False only when no child is possible at all, as at the delegation depth limit.
    pub read_only_child: bool,
    /// Why a writing child is `unavailable`, in plain words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Where a writing child of a session would work (decision 0161).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WritingChild {
    /// A new worktree of its own, on a branch from the parent workspace's branch.
    OwnWorktree,
    /// The parent's plain folder, shared with the parent's delegation tree.
    SharedFolder,
    /// No writing child is possible; `reason` says why.
    Unavailable,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Whether a session can be resumed (decision 0160). `code` and `reason` say why not when `available` is false.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeState {
    pub available: bool,
    /// `agent.resume_unavailable` or `agent.resume_not_found`, the code `agent.resume` would refuse with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl ResumeState {
    /// A session `agent.resume` would start a run of.
    #[must_use]
    pub fn available() -> Self {
        Self {
            available: true,
            code: None,
            reason: None,
        }
    }

    /// A session `agent.resume` refuses with `code`, saying `reason`.
    #[must_use]
    pub fn refused(code: &str, reason: impl Into<String>) -> Self {
        Self {
            available: false,
            code: Some(code.to_owned()),
            reason: Some(reason.into()),
        }
    }
}

/// Where a session's mode came from (decision 0083).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeSource {
    /// No mode was chosen: MassOrca passed no mode flag and the vendor CLI started in its own default.
    VendorDefault,
    /// A mode was chosen at start, at resume or with `agent.set_mode`.
    Chosen,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// The state of an account (decision 0059). A value this build does not know reads as `unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountState {
    SignedIn,
    Expired,
    NeedsSignIn,
    NotSetUp,
    /// Made from the single home of 0056, until its first check.
    Migrated,
    /// The vendor reports another identity than the one recorded.
    Conflict,
    /// The kind declares no check, or the value is unknown to this build.
    #[serde(other)]
    Unknown,
}

/// How an account came to be (decision 0059). A value this build does not know, or a summary from a peer that omits the
/// field, reads as `unknown`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountOrigin {
    /// The service ran the sign-in.
    SignIn,
    /// The person's files were imported.
    Import,
    /// Made from the single home of 0056, or adopted from a home that had no row.
    Migrated,
    /// A value from a newer peer that this build does not know. Also what a peer that omits the field decodes to.
    #[default]
    #[serde(other)]
    Unknown,
}

/// One account of a kind, as `agent.accounts.*` returns it. Its files are never part of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSummary {
    pub id: String,
    pub label: String,
    pub state: AccountState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
    /// Whether this is the kind's active account.
    pub active: bool,
    /// The projects that chose this account, by id.
    pub projects: Vec<String>,
    /// How the account came to be. A peer that omits it reads as `unknown`.
    #[serde(default)]
    pub origin: AccountOrigin,
}

/// Params of `agent.accounts.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountsListParams {
    pub kind: String,
}

/// Result of `agent.accounts.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountsList {
    pub accounts: Vec<AccountSummary>,
}

/// Params of `agent.accounts.add`, the sign-in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountsAddParams {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Result of `agent.accounts.add`: the new account and the terminal that runs the sign-in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountsAdded {
    pub account: AccountSummary,
    pub terminal_id: String,
}

/// Params of `agent.accounts.sign_in`: the existing account to sign in again, as `accounts list` names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountsSignInParams {
    pub kind: String,
    pub account: String,
}

/// Params of `agent.accounts.import`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountsImportParams {
    pub kind: String,
    /// The folder holding every declared file. Absent copies each file from where the kind's CLI keeps it (capability
    /// `agent-import-detect/1`, decision 0097).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// What an import did: `imported`, `failed` (with a reason), or `not_found` (a required file is missing; the reason names
/// each one and where it was looked for).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportResult {
    Imported,
    Failed,
    NotFound,
}

/// Result of `agent.accounts.import`. Never shows a secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountsImported {
    pub result: ImportResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<AccountSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The existing account this import updated, when the login was already that account (decision 0098): it got the
    /// imported files and kept its id, label, active mark and project choices. `account` is then that account. Absent for
    /// a new account, so an older peer reads the result as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
}

/// Params of `agent.accounts.import_candidates` (decision 0097). Without `kind`, every kind that declares credential
/// files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportCandidatesParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// Result of `agent.accounts.import_candidates`. Names, flags and host paths only, never file contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportCandidates {
    pub candidates: Vec<ImportCandidate>,
}

/// Where an import without `from` would copy one kind's declared files from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportCandidate {
    pub kind: String,
    /// Every required file was found.
    pub ready: bool,
    /// The folder of the first required file. Omitted below `workspace` scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    pub files: Vec<CandidateFile>,
}

/// One declared file of an [`ImportCandidate`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateFile {
    pub name: String,
    pub required: bool,
    /// The import would accept it: a regular file, not a link, at most 1 MiB (a state file: its rule's cap, decision
    /// 0099), judged from its metadata alone.
    pub found: bool,
    /// Where it was looked for. Omitted below `workspace` scope, and when the kind declares no location for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The vendor's override variable, when it was set and used. Omitted below `workspace` scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
    /// Why the import would not use a file that is there (`a link`, `a directory`, `larger than 1 MiB`); absent when it
    /// was found or is not there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Params of `agent.accounts.activate`: makes an account the kind's active one, or the project's choice when `project`
/// is given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountsActivateParams {
    pub kind: String,
    pub account: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// Params of `agent.accounts.remove`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountsRemoveParams {
    pub kind: String,
    pub account: String,
}

/// Result of `agent.accounts.remove`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountsRemoved {
    pub kind: String,
    pub account: String,
    pub removed: bool,
}

/// Params of `agent.models` (capability `agent-models/1`, decision 0166). Without `account`, the kind's active account is
/// listed, or the vendor's usual home when the kind has none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentModelsParams {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    /// Ask the vendor CLI again instead of answering from the cache.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub refresh: bool,
}

/// Result of `agent.models`: the kind's declared models, then the models its vendor CLI listed for the account and the
/// declaration does not name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentModels {
    pub models: Vec<AgentModel>,
    /// When the listing behind `models` was fetched, in milliseconds since the Unix epoch. Absent when no listing has
    /// succeeded, such as a kind that declares no listing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at_ms: Option<u64>,
    /// Why the latest listing failed. The declared models are still listed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ModelListingError>,
}

/// One model a picker offers: declared by the kind, or listed by its vendor CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentModel {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub default: bool,
    /// The effort levels the model accepts, by id. Absent: every level of the kind. Empty: the model takes no effort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub efforts: Option<Vec<String>>,
    pub source: ModelSource,
}

/// Where a model came from: the kind's declaration, or its vendor CLI's listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSource {
    Declared,
    Discovered,
}

/// Why a model listing failed (decision 0166).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelListingFailure {
    /// The kind's program was not found.
    CliMissing,
    /// The account's check reports it is not signed in, so the picker can offer the sign-in command.
    NotSignedIn,
    /// The vendor CLI did not answer within the time limit.
    TimedOut,
    /// The vendor CLI failed, or its output could not be read.
    Failed,
    /// The vendor CLI listed no model MassOrca can use.
    NoModels,
}

/// A failed model listing: its code, and a message that names no path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelListingError {
    pub code: ModelListingFailure,
    pub message: String,
}

/// Params of `agent.accounts.check`. Without `account`, the kind's active account is checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountsCheckParams {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

/// Params of `agent.generate_title` (capability `agent-title/1`, decision 0167): the first prompt of a chat and, once the
/// agent has answered, its first reply. Without `account`, the kind's active account runs the one-shot form; `model` is
/// passed only by a kind that reuses its model flag for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerateTitleParams {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<String>,
}

/// Result of `agent.generate_title`: at most four lowercase words joined by `-`, at most 48 characters of `a-z`, `0-9` and
/// `-`. Never empty: a run that names no usable title is `agent.oneshot_failed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerateTitleResult {
    pub slug: String,
}

/// Payload of the `agent.accounts_changed` event, whose id is the kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountsChanged {
    pub kind: String,
    pub account: String,
    pub state: AccountState,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_account_summary_without_an_origin_decodes_as_unknown() {
        let summary: AccountSummary = serde_json::from_value(json!({
            "id": "0195c0de-0000-7000-8000-000000000002",
            "label": "work",
            "state": "signed_in",
            "active": true,
            "projects": []
        }))
        .expect("a summary from an older peer decodes");
        assert_eq!(summary.origin, AccountOrigin::Unknown);
    }

    #[test]
    fn an_account_origin_decodes_by_its_wire_name_and_a_newer_one_reads_as_unknown() {
        for (wire, origin) in [
            ("sign_in", AccountOrigin::SignIn),
            ("import", AccountOrigin::Import),
            ("migrated", AccountOrigin::Migrated),
            ("from_a_newer_peer", AccountOrigin::Unknown),
        ] {
            let decoded: AccountOrigin =
                serde_json::from_value(json!(wire)).expect("an origin decodes");
            assert_eq!(decoded, origin, "{wire}");
        }
    }
}

/// Params of `agent.retain`. `retained: false` clears the retain (`agent retain --off`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRetainParams {
    pub session: String,
    pub retained: bool,
}

/// Result of `agent.archive`: the archived tail of a released session's terminal output (decision 0090).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentArchive {
    pub session: String,
    /// Whether the session was released. A session that was not has no archive.
    pub released: bool,
    /// The newest output kept, at most 256 KiB of UTF-8, cut on a character boundary. Absent when there is no archive, or
    /// when `withheld` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Whether older output was cut to keep the bound.
    pub truncated: bool,
    /// The run holds vault secrets (decision 0071) and the caller is below host-admin scope, so the text is not sent.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub withheld: bool,
}

/// Params of `agent.archive_session` (capability `agent-archive/1`, decision 0163).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentArchiveSessionParams {
    pub session: String,
    /// True archives the session; false clears its archive.
    pub archived: bool,
}

/// Params of `agent.forget`. Without `remove_workspace` it forgets the session only, as before decision 0163.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentForgetParams {
    pub session: String,
    /// Also remove the session's workspace through `workspace.remove`, with its guards and codes, when no other session
    /// uses it (capability `agent-archive/1`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub remove_workspace: bool,
    /// Discard uncommitted and untracked changes of the workspace being removed, as `workspace.remove`'s `force`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub force: bool,
    /// Delete ignored files of the worktree being removed (build output), as `workspace.remove`'s `discard_ignored`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub discard_ignored: bool,
    /// What the preview the person confirmed said about uncommitted or untracked changes. `Some(false)` refuses with
    /// `workspace.dirty` when the worktree has such changes now, even with `force`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect_dirty: Option<bool>,
    /// Remove the workspace even when its archive script fails or times out, as `workspace.remove`'s `allow_failed_archive`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_failed_archive: bool,
}

/// Result of `agent.forget`: the forgotten session, with what became of its workspace when one was to be removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentForgotten {
    #[serde(flatten)]
    pub session: AgentSession,
    /// The workspace removal, when `remove_workspace` was asked and it succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_removed: Option<WorkspaceRemoved>,
    /// Why the workspace was not removed after the session was forgotten. Every check runs before the forget, so this
    /// is only a failure of the removal itself; the workspace is then as `workspace.remove` left it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_error: Option<WorkspaceRemovalError>,
}

/// A workspace removal that failed after its session was forgotten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceRemovalError {
    pub code: String,
    pub message: String,
}

/// Result of `agent.forget_preview` (capability `agent-archive/1`, decision 0163): what deleting the session with its
/// workspace would meet. Read from the same checks the delete runs; nothing changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentForgetPreview {
    pub session: String,
    /// Whether `agent.forget` would forget the session now.
    pub can_forget: bool,
    /// Why it would not, in plain words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forget_blocker: Option<String>,
    pub workspace: ForgetPreviewWorkspace,
}

/// The workspace part of an [`AgentForgetPreview`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForgetPreviewWorkspace {
    pub id: String,
    pub kind: WorkspaceKind,
    /// The workspace's directory; absent for a caller below `workspace` scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Whether `agent.forget` with `remove_workspace` and without `force` would remove it. A folder workspace is only
    /// forgotten; its files stay.
    pub removable: bool,
    /// Why it would not, in plain words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocker: Option<String>,
    /// The stable code of that refusal, such as `workspace.dirty` or `agent.busy`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocker_code: Option<String>,
    /// Uncommitted changes to tracked files, which only `force` discards.
    pub dirty: bool,
    /// Untracked files, which only `force` discards.
    pub untracked: bool,
    /// Ignored files that a removal would delete, which `agent.forget` never discards.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ignored_files: bool,
    /// Other sessions (live, exited or archived) of the workspace; while any is left, the workspace is not removed.
    pub other_sessions: Vec<String>,
    /// Teams whose workspace this is; while any is left, the workspace is not removed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_teams: Vec<String>,
    /// Tasks whose workspace this is; while any is left, the workspace is not removed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_tasks: Vec<String>,
    /// Terminals running in the workspace; while any runs, the workspace is not removed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub running_terminals: Vec<String>,
}

/// Params of `agent.rename` (capability `agent-archive/1`, decision 0163).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRenameParams {
    pub session: String,
    pub title: String,
}
