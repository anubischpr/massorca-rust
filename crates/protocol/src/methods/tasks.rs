//! Task methods: the tasks on a team's Board (capability `teams/1`, decision 0072). `task.start` is not built yet.
//!
//! Every enum here has an `Unknown` variant for values from a newer peer; fields are only ever added (0002).

use serde::{Deserialize, Serialize};

use crate::methods::agents::Outcome;

/// Params: [`TaskCreateParams`]. Result: [`Task`].
pub const CREATE: &str = "task.create";
/// Params: [`TaskListParams`]. Result: [`TaskList`], oldest first.
pub const LIST: &str = "task.list";
/// Params: [`TaskRef`]. Result: [`Task`].
pub const SHOW: &str = "task.show";
/// Params: [`TaskUpdateParams`]. Result: [`Task`].
pub const UPDATE: &str = "task.update";
/// Params: [`TaskRef`]. Result: [`Task`]. Refused while the task's agent session runs.
pub const DONE: &str = "task.done";
/// Params: [`TaskRef`]. Result: [`Task`], linked to the agent session that runs it. A failed start keeps the task
/// Open with the reason in `error`.
pub const START: &str = "task.start";
/// Params: [`TaskRef`]. Result: [`Task`]. Clears the outcome and error; refused while the agent session runs.
pub const REOPEN: &str = "task.reopen";
/// Params: [`TaskStopParams`]. Result: [`Task`]. Stops the agent session the task runs in; the task keeps the session
/// and shows Stopped. Refused with `task.not_running` when no session runs.
pub const STOP: &str = "task.stop";
/// Params: [`TaskRef`]. Result: [`Task`], the removed task. Refused while the agent session runs.
pub const REMOVE: &str = "task.rm";

/// Where a task came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskOrigin {
    /// Created by the Conference room.
    Room,
    /// Created from the command line or a client without a room.
    #[default]
    Cli,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// The Board column a task is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Column {
    Open,
    Running,
    Waiting,
    Done,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

impl Column {
    /// The four columns, in Board order.
    pub const ALL: [Column; 4] = [Column::Open, Column::Running, Column::Waiting, Column::Done];

    /// The column's title on the Board.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Running => "Running",
            Self::Waiting => "Waiting for you",
            Self::Done => "Done",
            Self::Unknown => "Unknown",
        }
    }
}

/// The state word of a task's card. The column follows from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardState {
    NotStarted,
    Working,
    Waiting,
    Done,
    Stuck,
    Stopped,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

impl CardState {
    /// The word the card shows.
    pub const fn label(self) -> &'static str {
        match self {
            Self::NotStarted => "Not started",
            Self::Working => "Working",
            Self::Waiting => "Waiting for you",
            Self::Done => "Done",
            Self::Stuck => "Stuck",
            Self::Stopped => "Stopped",
            Self::Unknown => "Unknown",
        }
    }
}

/// Params of `task.create`. `origin` defaults to `cli`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCreateParams {
    /// Team id or exact name.
    pub team: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// A bot of the team: id, or exact name within the team.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot: Option<String>,
    /// A project of the team: id, or exact name within the team.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<TaskOrigin>,
}

/// Filters of `task.list`. Each one given narrows the list; the team and project selectors resolve as elsewhere.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TaskListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<Column>,
}

/// Result of `task.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskList {
    pub tasks: Vec<Task>,
}

/// Selects one task by id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRef {
    pub task: String,
}

/// Params of `task.stop`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskStopParams {
    pub task: String,
    /// Skip the grace period and end the agent's processes at once, as `agent.stop` does.
    #[serde(default)]
    pub force: bool,
}

/// Params of `task.update`. An empty `bot` or `project` clears that link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskUpdateParams {
    pub task: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// One task on a team's Board, with the state its agent session gives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub team_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_id: Option<String>,
    pub title: String,
    pub body: String,
    pub origin: TaskOrigin,
    /// The agent session that runs the task; set only after its prompt was delivered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// The task's own final state, once latched; it outlives the session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,
    /// Why the last start failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The protocol code of `error`, such as `agent.not_installed` or `bot.busy`, for the sentence a client shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// The workspace the last start ran in: the task's own worktree, a linked folder, or the team folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// The branch of the task's own worktree, where its result is; set only for a linked git project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub column: Column,
    pub state: CardState,
    /// The session's terminal cannot be reached, so the state is the last one the agent reported.
    #[serde(default, skip_serializing_if = "is_false")]
    pub last_known: bool,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

fn is_false(value: &bool) -> bool {
    !*value
}
