//! The transcript item vocabulary of a chat session's stream, mirrored from the chat protocol (decision 0101, D3-D4).
//! It is not imported from the protocol crate, because that branch is not on main yet; the service's decoder maps its
//! values onto these types.

use massorca_protocol::methods::agents::Activity;

/// A position in one session's journal. A rebuilt journal has a new epoch, so a cursor from another epoch is stale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatCursor {
    pub epoch: String,
    pub seq: u64,
}

/// One transcript item. `position` orders the timeline and never changes; `seq` says when the item last changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatItem {
    pub id: String,
    pub position: u64,
    pub seq: u64,
    /// The turn item the item belongs to; absent on a turn item itself.
    pub turn: Option<String>,
    /// The subagent item whose run produced the item; absent for the session's own agent.
    pub parent: Option<String>,
    pub at_ms: i64,
    pub body: ChatBody,
}

/// What an item is. `Unknown` stands for a type from a newer peer: readers skip it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatBody {
    UserMessage {
        text: String,
    },
    AssistantMessage {
        text: String,
        state: MessageState,
    },
    Thinking {
        text: String,
        state: MessageState,
    },
    ToolUse(ToolUse),
    Plan {
        entries: Vec<PlanEntry>,
    },
    Diff {
        path: String,
        patch: String,
        truncated: bool,
    },
    Subagent {
        tool_use_id: String,
        description: String,
        agent_type: Option<String>,
        status: SubagentStatus,
    },
    Approval(Approval),
    Notice {
        level: NoticeLevel,
        text: String,
    },
    Turn(Turn),
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageState {
    Running,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolUse {
    pub tool_use_id: String,
    pub name: String,
    pub input_summary: String,
    pub status: ToolStatus,
    pub output_summary: Option<String>,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolStatus {
    Started,
    Updated,
    Completed,
    Failed,
    Interrupted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanEntry {
    pub text: String,
    pub status: PlanStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubagentStatus {
    Running,
    Completed,
    Failed,
    Interrupted,
}

/// A permission request. Only `Allowed` authorizes the tool; `Unknown` (a value from a newer peer) is never allowed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Approval {
    pub request_id: String,
    pub tool: String,
    pub input_summary: String,
    pub truncated: bool,
    pub tool_use_id: Option<String>,
    pub can_always: bool,
    pub state: ApprovalState,
    pub scope: Option<ApprovalScope>,
    pub settled_at_ms: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalState {
    Pending,
    Allowed,
    Denied,
    Cancelled,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalScope {
    Once,
    Always,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeLevel {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    pub state: TurnState,
    pub user_item: Option<String>,
    pub completed_at_ms: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnState {
    Running,
    Completed,
    Interrupted,
    Unverifiable,
}

/// A stream event, as `agent.chat.subscribe` sends it. `Unknown` is an event from a newer peer.
#[derive(Clone, Debug, PartialEq)]
pub enum ChatEvent {
    Snapshot(Box<ChatSnapshot>),
    Batch(ChatBatch),
    Reset { reason: ResetReason },
    End { reason: EndReason },
    Unknown,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatSnapshot {
    pub cursor: ChatCursor,
    pub items: Vec<ChatItem>,
    pub has_older: bool,
    pub latest_turn: Option<ChatItem>,
    pub activity: Option<Activity>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatBatch {
    pub cursor: ChatCursor,
    pub updates: Vec<ChatUpdate>,
    pub activity: Option<Activity>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatUpdate {
    /// The item's whole current state: replace the held item with the same id, or add it.
    Item(ChatItem),
    /// Text added to an assistant message or thinking item that held exactly `offset` bytes before it.
    Append {
        id: String,
        seq: u64,
        offset: u64,
        text: String,
    },
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetReason {
    TooOld,
    EpochChanged,
    Overflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    Forgotten,
    ShuttingDown,
}
