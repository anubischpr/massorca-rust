//! Structured chat sessions: an agent session whose run speaks a structured protocol with the service instead of drawing
//! a terminal (capability `agent-chat/1`, decision 0101). The first slice supports Claude Code only.
//!
//! A chat session is an ordinary agent session ([`AgentSession`](crate::methods::agents::AgentSession)): `agent.show`,
//! `agent.list`, `agent.watch`, `agent.stop`, `agent.resume`, `agent.prompt`, `agent.queue*`, `agent.set_mode` and
//! `agent.forget` work on it as on a terminal session. The methods here add what only a structured run has: the
//! transcript stream, history paging, approvals and an interrupt that keeps the session.
//!
//! Every enum here has an `Unknown` variant for values from a newer peer; fields are only ever added (0002).

use serde::{Deserialize, Serialize};

use crate::codes;
use crate::methods::agents::{Activity, AgentStartParams, Outcome};

/// Params: [`AgentChatStartParams`]. Result: [`AgentSession`](crate::methods::agents::AgentSession). Refused with
/// [`CHAT_UNSUPPORTED`] when the kind cannot run as a chat; the client then offers a terminal session (`agent.start`).
pub const START: &str = "agent.chat.start";
/// Params: [`ChatSubscribeParams`]. Result: `{ stream }`, as `events.subscribe`; then [`ChatEvent`]s.
pub const SUBSCRIBE: &str = "agent.chat.subscribe";
/// Params: [`ChatHistoryParams`]. Result: [`ChatHistoryPage`].
pub const HISTORY: &str = "agent.chat.history";
/// Params: [`ChatApprovalAnswerParams`]. Result: [`ChatItem`], the approval item after the answer.
pub const APPROVAL_ANSWER: &str = "agent.chat.approval.answer";
/// Params: [`AgentSessionRef`](crate::methods::agents::AgentSessionRef). Result: [`ChatInterrupted`].
pub const INTERRUPT: &str = "agent.chat.interrupt";

/// `agent.chat.start` refused; [`codes::AGENT_CHAT_UNSUPPORTED`], kept here beside its method.
pub const CHAT_UNSUPPORTED: &str = codes::AGENT_CHAT_UNSUPPORTED;
/// A chat method was called on a session that does not run as a chat; [`codes::AGENT_NOT_CHAT`].
pub const NOT_CHAT: &str = codes::AGENT_NOT_CHAT;
/// The approval is not pending; [`codes::AGENT_APPROVAL_SETTLED`].
pub const APPROVAL_SETTLED: &str = codes::AGENT_APPROVAL_SETTLED;
/// The approval changed since the `seq` the answer names; [`codes::AGENT_APPROVAL_CHANGED`].
pub const APPROVAL_CHANGED: &str = codes::AGENT_APPROVAL_CHANGED;
/// A cursor from another journal epoch; [`codes::AGENT_CURSOR_STALE`].
pub const CURSOR_STALE: &str = codes::AGENT_CURSOR_STALE;

/// Items a snapshot or a history page holds when the caller gives no limit.
pub const DEFAULT_PAGE_ITEMS: u32 = 40;
/// Most items one snapshot or history page holds.
pub const MAX_PAGE_ITEMS: u32 = 200;
/// Most bytes of a summary or patch an item carries (`input_summary`, `output_summary`, `patch`); longer text is cut on
/// a character boundary and the item says `truncated`.
pub const MAX_SUMMARY_BYTES: usize = 16 * 1024;

/// How a session's run talks to the service. Absent on a session record means `terminal`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunType {
    /// The vendor CLI draws its own screen in a terminal.
    #[default]
    Terminal,
    /// The vendor CLI speaks a structured protocol; the service keeps the transcript.
    Chat,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

impl RunType {
    /// Whether the value is the default, which a session record leaves out.
    pub fn is_terminal(&self) -> bool {
        *self == Self::Terminal
    }
}

/// Whether a kind can run as a chat, as `agent.kinds` lists it in [`AgentKind`](crate::methods::agents::AgentKind)'s
/// `chat`. Absent from the kind means no chat (a kind without chat support, or an older peer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindChat {
    /// `agent.chat.start` accepts the kind now.
    pub supported: bool,
    /// Why not, when `supported` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<ChatUnsupportedReason>,
    /// The CLI version the service probed, when it read one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The supported version range, for display (for example `>=2.1.0, <2.3.0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<String>,
}

/// Why a kind cannot run as a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatUnsupportedReason {
    /// The kind's program does not resolve, so no version could be probed.
    NotDetected,
    /// The program ran but its version could not be read.
    VersionUnknown,
    /// The probed version is outside `range`.
    VersionOutsideRange,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Params of `agent.chat.start`: the `agent.start` params, at the top level. `cols`, `rows` and `args` do not apply to
/// a chat and are refused with `request.invalid_params`; every other field means what it means for `agent.start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentChatStartParams {
    #[serde(flatten)]
    pub start: AgentStartParams,
}

/// A position in one session's transcript journal. `seq` grows by one for every change; 0 is "before the first change".
/// `epoch` names the journal: a journal that was rebuilt gets a new epoch, and a cursor from another epoch is stale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatCursor {
    pub epoch: String,
    pub seq: u64,
}

/// Params of `agent.chat.subscribe`. Without `after` (or with a stale one), the stream opens with a snapshot of the
/// newest page; with a current `after`, it opens with one batch of everything changed after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSubscribeParams {
    pub session: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<ChatCursor>,
    /// Items of the opening snapshot; [`DEFAULT_PAGE_ITEMS`] when absent, at most [`MAX_PAGE_ITEMS`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// One item of an `agent.chat.subscribe` stream, tagged by `event`. Readers ignore events they do not know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ChatEvent {
    /// The first event without a current `after`, and the event after a `reset`: the newest page of the transcript.
    /// It replaces everything the client holds.
    Snapshot(Box<ChatSnapshot>),
    /// What changed since the previous event. Applying a batch twice converges.
    Batch(ChatBatch),
    /// The client's position can no longer be served (it fell behind the replay window, or the journal was rebuilt).
    /// A snapshot follows on the same stream.
    Reset { reason: ResetReason },
    /// The stream ends; nothing follows.
    End { reason: EndReason },
    /// An event this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// The newest page of a transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSnapshot {
    /// The journal head: the cursor of the newest change, to resubscribe `after`.
    pub cursor: ChatCursor,
    /// The newest items, oldest first by `position`.
    pub items: Vec<ChatItem>,
    /// Older items exist; page them with `agent.chat.history`.
    pub has_older: bool,
    /// The session's newest turn item, even when it is older than the page (a long turn keeps the position it opened
    /// at). Absent: no turn yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_turn: Option<ChatItem>,
    /// The session's activity, as `agent.show` reports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<Activity>,
}

/// The changes after the previous event, in journal order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatBatch {
    /// The cursor after the last update.
    pub cursor: ChatCursor,
    pub updates: Vec<ChatUpdate>,
    /// The session's activity, when it changed with these updates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<Activity>,
}

/// One change, tagged by `op`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ChatUpdate {
    /// An item was added or changed: its whole current state. Replace the item with the same `id`, or add it.
    Item { item: ChatItem },
    /// Text appended to the `text` of an `assistant_message` or `thinking` item. `offset` is the byte length of the
    /// text before the append: apply it when the held text has exactly that length, skip it when the held text is
    /// already longer (a repeat), and resubscribe when it is shorter (a gap).
    Append {
        id: String,
        seq: u64,
        offset: u64,
        text: String,
    },
    /// An update this build does not know; never sent by this build. Resubscribe to get the item's state.
    #[serde(other)]
    Unknown,
}

/// Why a stream was reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResetReason {
    /// `after` is older than the changes the service keeps for replay.
    TooOld,
    /// `after` names another journal epoch.
    EpochChanged,
    /// The subscriber fell behind the stream's buffer.
    Overflow,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Why a stream ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    /// The session was forgotten.
    Forgotten,
    /// The service is shutting down.
    ShuttingDown,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// One transcript item. `id` is stable for the item's life; `position` orders the timeline and never changes; `seq`
/// says when the item last changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatItem {
    pub id: String,
    /// The journal `seq` at which the item was added; the timeline is ordered by it.
    pub position: u64,
    /// The journal `seq` at which the item last changed.
    pub seq: u64,
    /// The id of the turn item the item belongs to. Absent on a turn item and on items outside any turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<String>,
    /// The id of the `subagent` item whose run produced the item. Absent for the session's own agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Service clock when the item was added.
    pub at_ms: i64,
    #[serde(flatten)]
    pub body: ChatItemBody,
}

/// What an item is, tagged by `type`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatItemBody {
    /// A prompt the person sent (`agent.chat.start`, `agent.prompt` or a delivered queue item).
    UserMessage { text: String },
    /// Text the agent wrote. It grows by `append` updates while `state` is `running`.
    AssistantMessage { text: String, state: MessageState },
    /// The agent's reasoning, when the CLI shares it.
    Thinking { text: String, state: MessageState },
    /// One tool call.
    ToolUse(ToolUse),
    /// The agent's plan or checklist, as a whole list each time it changes.
    Plan {
        entries: Vec<PlanEntry>,
        /// The file the plan was written to, when the agent wrote it to one (a path as the agent names it).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        document: Option<String>,
    },
    /// A file change the agent made.
    Diff {
        path: String,
        /// What happened to the file; `modified` when a peer leaves it out.
        #[serde(default)]
        status: DiffStatus,
        /// The file's earlier path, for a `renamed` file.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old_path: Option<String>,
        /// A unified diff, at most [`MAX_SUMMARY_BYTES`].
        patch: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        truncated: bool,
    },
    /// A sub-agent the agent started; the items it produces name this item's `id` as their `parent`.
    Subagent {
        /// The id of the tool call that started it.
        tool_use_id: String,
        /// What the agent asked it to do.
        description: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_type: Option<String>,
        status: SubagentStatus,
    },
    /// A permission request from the agent (decision 0101, D6).
    Approval(Approval),
    /// A line from the service or the CLI that is not part of the conversation (a retry, a refusal, a failure).
    Notice { level: NoticeLevel, text: String },
    /// One turn: a prompt and everything the agent did for it. The item's `id` is the turn id.
    Turn(Turn),
    /// An item this build does not know; never sent by this build. Readers skip it.
    #[serde(other)]
    Unknown,
}

/// Whether the agent is still writing a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageState {
    Running,
    Completed,
    /// A value from a newer peer that this build does not know; never sent by this build. Read it as `completed`.
    #[serde(other)]
    Unknown,
}

/// One tool call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolUse {
    /// The vendor's id of the call.
    pub tool_use_id: String,
    /// The tool's name, as the vendor names it (`Bash`, `Edit`, an MCP tool's full name).
    pub name: String,
    /// The call's input as text, at most [`MAX_SUMMARY_BYTES`].
    pub input_summary: String,
    pub status: ToolStatus,
    /// The call's output as text, at most [`MAX_SUMMARY_BYTES`]; absent until there is output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_summary: Option<String>,
    /// `input_summary` or `output_summary` was cut.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

/// Where a tool call is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Started,
    /// Progress arrived; `output_summary` may have grown.
    Updated,
    Completed,
    Failed,
    /// The turn was interrupted before the call ended.
    Interrupted,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// What a [`ChatItemBody::Diff`] did to its file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffStatus {
    /// The file is new.
    Added,
    /// The file existed and changed.
    #[default]
    Modified,
    /// The file was removed.
    Deleted,
    /// The file moved from `old_path`.
    Renamed,
    /// A value from a newer peer that this build does not know; never sent by this build. Read it as `modified`.
    #[serde(other)]
    Unknown,
}

/// One entry of a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanEntry {
    pub text: String,
    pub status: PlanStatus,
}

/// Where a plan entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Pending,
    InProgress,
    Completed,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Where a sub-agent is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStatus {
    Running,
    Completed,
    Failed,
    Interrupted,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// A permission request. Only `allowed` authorizes the tool; every other state leaves it unauthorized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approval {
    /// The id `agent.chat.approval.answer` takes.
    pub request_id: String,
    /// The tool the agent wants to run.
    pub tool: String,
    /// The tool's input as text, at most [`MAX_SUMMARY_BYTES`].
    pub input_summary: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
    /// The tool call the request is for, when the CLI names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    /// The CLI offered rules that would allow such calls from now on, so the `always` scope is accepted: true only when
    /// `suggestions` lists at least one.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub can_always: bool,
    /// What an `always` answer writes, each listed so the person sees it before accepting. Only these are written; the
    /// service drops every other change the CLI offered (a mode switch, another destination).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<ApprovalSuggestion>,
    pub state: ApprovalState,
    /// The scope of an `allowed` answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ApprovalScope>,
    /// Service clock when the request left `pending`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settled_at_ms: Option<i64>,
}

/// One change an `always` answer writes (decision 0101, as amended): the CLI's own type for it, the rules as text, and
/// where they are written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalSuggestion {
    /// The CLI's type of change, for example `addRules`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Each rule as `Tool` or `Tool(rule)`, for example `Bash(touch probe.txt)`.
    pub rules: Vec<String>,
    /// Where the rules are written, for example `session` or `localSettings`.
    pub destination: String,
}

/// Where a permission request is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalState {
    /// Waiting for an answer; the agent is `waiting`.
    Pending,
    Allowed,
    Denied,
    /// The CLI withdrew the request, the turn was interrupted, or the run ended before an answer. Never authorizes.
    Cancelled,
    /// A value from a newer peer that this build does not know; never sent by this build. Never read as allowed.
    #[serde(other)]
    Unknown,
}

/// A person's answer to a permission request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Allow,
    Deny,
}

/// How far an `allow` reaches.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalScope {
    /// This call only.
    #[default]
    Once,
    /// This call, and the rules the CLI offered with the request (only when the item says `can_always`).
    Always,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Kind of a notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeLevel {
    Info,
    Warning,
    Error,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// One turn's lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Turn {
    pub state: TurnState,
    /// The CLI's own verdict, when it gave one. Absent means unknown, never success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,
    /// The `user_message` item that opened the turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_item: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at_ms: Option<i64>,
}

/// Where a turn is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnState {
    Running,
    /// The CLI reported the turn's end.
    Completed,
    /// `agent.chat.interrupt` or `agent.stop` ended it.
    Interrupted,
    /// The run ended with the turn open and nothing says how (a crash, a service restart).
    Unverifiable,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Params of `agent.chat.history`: the items before `before`, newest last. Without `before`, the newest page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatHistoryParams {
    pub session: String,
    /// A `position` in the given epoch, usually the oldest item the client holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<ChatCursor>,
    /// [`DEFAULT_PAGE_ITEMS`] when absent, at most [`MAX_PAGE_ITEMS`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Result of `agent.chat.history`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatHistoryPage {
    pub epoch: String,
    /// Oldest first by `position`.
    pub items: Vec<ChatItem>,
    /// Items older than the page exist.
    pub has_older: bool,
}

/// Params of `agent.chat.approval.answer`. The first answer to a pending request wins; any later one, and any answer to
/// a cancelled request, is refused with [`APPROVAL_SETTLED`]. With `seq`, the answer applies only to the item as it was
/// at that `seq`: a request the CLI sent again since (with new input) is refused with [`APPROVAL_CHANGED`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatApprovalAnswerParams {
    pub session: String,
    pub request_id: String,
    pub decision: ApprovalDecision,
    /// For `allow` only; `once` when absent. `always` on a request without `can_always` is refused with
    /// `request.invalid_params`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ApprovalScope>,
    /// For `deny` only: what the agent is told, so it can change course.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// The `seq` of the approval item the person answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
}

/// Result of `agent.chat.interrupt`. The session and its run stay; only the running turn ends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatInterrupted {
    /// A turn was running and was asked to stop. False when no turn ran: the call does nothing and is not an error.
    pub interrupted: bool,
    /// The interrupted turn's id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<String>,
}
