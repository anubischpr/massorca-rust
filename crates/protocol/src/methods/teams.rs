//! Team methods: teams, their bots and projects, and the Board (capability `teams/1`, decision 0072).
//!
//! A team is selected by id or exact name. A bot or project is selected by id, or by exact name within its team.
//! Every enum here has an `Unknown` variant for values from a newer peer; fields are only ever added (0002).

use serde::{Deserialize, Serialize};

use crate::methods::projects::ProjectKind;
use crate::methods::tasks::{CardState, Column};

/// Params: [`TeamCreateParams`]. Result: [`Team`].
pub const CREATE: &str = "team.create";
/// Params: none. Result: [`TeamList`], ordered by name.
pub const LIST: &str = "team.list";
/// Params: [`TeamRef`]. Result: [`Team`].
pub const SHOW: &str = "team.show";
/// Params: [`TeamUpdateParams`]. Result: [`Team`].
pub const UPDATE: &str = "team.update";
/// Params: [`TeamRemoveParams`]. Result: [`Team`], the removed team. Refused while a task runs unless `force`.
pub const REMOVE: &str = "team.rm";
/// Params: [`TeamRef`]. Result: [`BotList`], in creation order.
pub const BOT_LIST: &str = "team.bot_list";
/// Params: [`BotAddParams`]. Result: [`Bot`].
pub const BOT_ADD: &str = "team.bot_add";
/// Params: [`BotUpdateParams`]. Result: [`Bot`].
pub const BOT_UPDATE: &str = "team.bot_update";
/// Params: [`BotRemoveParams`]. Result: [`Bot`], the removed bot. Refused while it owns open tasks unless `unassign`.
pub const BOT_REMOVE: &str = "team.bot_rm";
/// Params: [`TeamRef`]. Result: [`TeamProjectList`], in creation order.
pub const PROJECT_LIST: &str = "team.project_list";
/// Params: [`TeamProjectAddParams`]. Result: [`TeamProject`].
pub const PROJECT_ADD: &str = "team.project_add";
/// Params: [`TeamProjectUpdateParams`]. Result: [`TeamProject`].
pub const PROJECT_UPDATE: &str = "team.project_update";
/// Params: [`TeamProjectRemoveParams`]. Result: [`TeamProject`], the removed project.
pub const PROJECT_REMOVE: &str = "team.project_rm";
/// Params: [`BoardParams`]. Result: [`Board`].
pub const BOARD: &str = "team.board";
/// Params: [`TeamMemoryListParams`]. Result: [`TeamMemoryList`], newest first.
pub const MEMORY_LIST: &str = "team.memory_list";
/// Params: [`TeamMemoryAddParams`]. Result: [`TeamMemoryLine`].
pub const MEMORY_ADD: &str = "team.memory_add";
/// Params: [`TeamMemoryRemoveParams`]. Result: [`TeamMemoryLine`], the removed line.
pub const MEMORY_REMOVE: &str = "team.memory_rm";

/// Routines: a schedule that starts a bot on a task each time it is due (decision 0072 amendment).
pub const ROUTINE_LIST: &str = "team.routine_list";
pub const ROUTINE_ADD: &str = "team.routine_add";
pub const ROUTINE_UPDATE: &str = "team.routine_update";
pub const ROUTINE_REMOVE: &str = "team.routine_rm";
pub const ROUTINE_PAUSE: &str = "team.routine_pause";
pub const ROUTINE_RESUME: &str = "team.routine_resume";
pub const ROUTINE_RUNS: &str = "team.routine_runs";
/// Makes one run now without moving the schedule.
pub const ROUTINE_RUN: &str = "team.routine_run";

/// The shape a team is drawn with. Cosmetic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TeamCharacter {
    #[default]
    Triangle,
    Square,
    Circle,
    Diamond,
    Hexagon,
    Star,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// How a team's bots run (decision 0072, amendment "team access"; decision 0083).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TeamAccess {
    /// Bots run in their kind's bypass mode, without asking for permission. A new team's access when none is given.
    #[default]
    Full,
    /// Bots run in their kind's own default mode, which may ask for permission. Absent from older hosts.
    Default,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// A team: its bots, projects and Board are looked up through it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub character: TeamCharacter,
    pub access: TeamAccess,
    /// The team has a folder Workspaces can use.
    pub workspaces: bool,
    pub created_at_ms: i64,
}

/// Params of `team.create`. Unset options take the defaults: no purpose, the triangle, full access, no workspaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamCreateParams {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<TeamCharacter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<TeamAccess>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspaces: Option<bool>,
}

/// Selects one team by id or name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamRef {
    pub team: String,
}

/// Params of `team.update`. Only the fields given change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamUpdateParams {
    pub team: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<TeamCharacter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<TeamAccess>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspaces: Option<bool>,
}

/// Params of `team.rm`. `force` removes a team whose task is running; its agent sessions stay with the agents domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamRemoveParams {
    pub team: String,
    #[serde(default)]
    pub force: bool,
}

/// Result of `team.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamList {
    pub teams: Vec<Team>,
}

/// Result of `team.bot_list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotList {
    pub bots: Vec<Bot>,
}

/// Result of `team.project_list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamProjectList {
    pub projects: Vec<TeamProject>,
}

/// A bot of a team. At most one bot per team is the lead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bot {
    pub id: String,
    pub team_id: String,
    pub name: String,
    pub job: String,
    /// The agent kind the bot runs as, such as `claude`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    pub lead: bool,
    pub created_at_ms: i64,
}

/// Params of `team.bot_add`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotAddParams {
    pub team: String,
    pub name: String,
    pub job: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default)]
    pub lead: bool,
}

/// Params of `team.bot_update`. Only the fields given change; an empty `model` or `effort` clears it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotUpdateParams {
    pub team: String,
    pub bot: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// `true` makes the bot the lead, which is refused while another bot leads; `false` clears the lead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead: Option<bool>,
}

/// Params of `team.bot_rm`. `unassign` removes a bot that owns open tasks, which then have no bot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotRemoveParams {
    pub team: String,
    pub bot: String,
    #[serde(default)]
    pub unassign: bool,
}

/// A project of a team, with its own Board and Brief, and the Workspaces project its code comes from when linked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamProject {
    pub id: String,
    pub team_id: String,
    pub name: String,
    pub goal: String,
    pub brief: String,
    pub created_at_ms: i64,
    /// The Workspaces project the code comes from, by id. Unset when no project is linked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_project: Option<String>,
    /// The name of the linked project. Unset when the link is absent or the project is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_project_name: Option<String>,
    /// Whether the linked project is a git repository or a folder. Unset like `source_project_name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<ProjectKind>,
}

/// Params of `team.project_add`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamProjectAddParams {
    pub team: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    /// A Workspaces project (id, exact name or path) the code comes from. Unset or blank links none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_project: Option<String>,
}

/// Params of `team.project_update`. Only the fields given change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamProjectUpdateParams {
    pub team: String,
    pub project: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    /// The Workspaces project (id, exact name or path) the code comes from. An empty value clears the link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_project: Option<String>,
}

/// Params of `team.project_rm`. The project's tasks stay on the team's Board without a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamProjectRemoveParams {
    pub team: String,
    pub project: String,
}

/// Params of `team.board`: the team's Board, or only one project's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardParams {
    pub team: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// A team's Board: the four columns, in order, each with its cards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub team_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub columns: Vec<BoardColumn>,
}

/// One column of a Board and its cards, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardColumn {
    pub column: Column,
    pub cards: Vec<BoardCard>,
}

/// One task as a Board card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardCard {
    /// The task id.
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_id: Option<String>,
    /// The bot's name, for display.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot: Option<String>,
    pub state: CardState,
    /// The state is the last one the agent reported, because its terminal cannot be reached.
    #[serde(default, skip_serializing_if = "is_false")]
    pub last_known: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The protocol code of `error`, such as `agent.not_installed` or `bot.busy`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// The workspace the last start ran in: the task's own worktree, a linked folder, or the team folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// The branch of the task's own worktree, where its result is; set only for a linked git project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// When the task last changed, Unix milliseconds.
    pub at_ms: i64,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Where a memory line came from: a bot during a task, or the owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemorySource {
    Task,
    Person,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// A line of a team's memory, in the team scope or in one bot's. Lists are newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMemoryLine {
    pub id: String,
    pub team_id: String,
    /// The bot whose scope the line is in; absent for the team scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_id: Option<String>,
    /// At most 500 characters.
    pub text: String,
    pub source: MemorySource,
    /// The task that wrote a `task` line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub created_at_ms: i64,
}

/// Params of `team.memory_list`. Without `bot`, the lines of the whole team; with it, only that bot's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMemoryListParams {
    pub team: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot: Option<String>,
}

/// Result of `team.memory_list`, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMemoryList {
    pub lines: Vec<TeamMemoryLine>,
}

/// Params of `team.memory_add`. A `task` makes the line a task line; without it, the owner wrote the line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMemoryAddParams {
    pub team: String,
    /// The bot whose scope the line joins: id, or exact name within the team. The team scope when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot: Option<String>,
    pub text: String,
    /// The team's task that wrote the line: its id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
}

/// Params of `team.memory_rm`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMemoryRemoveParams {
    pub team: String,
    /// The line's id.
    pub line: String,
}

/// How often a routine runs. A value this build does not know decodes as `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutineCadence {
    Daily,
    Weekly,
    Monthly,
    Once,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// The day a weekly routine runs. A value this build does not know decodes as `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutineWeekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// A routine of a team: a schedule that starts its bot on a task each time it is due. The zone decides which wall-clock
/// time the schedule means; the instants are UTC epoch milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Routine {
    pub id: String,
    pub team_id: String,
    /// The bot the runs' tasks are for; absent once that bot is removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_id: Option<String>,
    pub name: String,
    pub prompt: String,
    pub cadence: RoutineCadence,
    /// Wall-clock `HH:MM`, 24-hour.
    pub time: String,
    /// The day a weekly routine runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekday: Option<RoutineWeekday>,
    /// The day of the month, 1 to 28, of a monthly routine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub month_day: Option<u8>,
    /// `YYYY-MM-DD`, the date of a once routine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_date: Option<String>,
    /// An IANA zone name; absent means the host's zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    /// For a routine without `zone`, the zone its runs use on the host: the host zone's name, or `UTC` when the host's
    /// zone cannot be read. Absent from older hosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_used: Option<String>,
    /// A paused routine has no next run; resuming schedules the next one from then, so no missed run is made up.
    pub paused: bool,
    /// Whether its scheduled runs receive vault secrets: the host-admin status of the caller that last added or updated it.
    pub starts_as_admin: bool,
    /// The next run; absent while paused or once the one-time routine has run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_run_at_ms: Option<i64>,
    pub created_at_ms: i64,
}

/// Params of `team.routine_add`. A weekday, month day or date is kept only when its cadence uses it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineAddParams {
    pub team: String,
    /// The bot the runs' tasks are for: id, or exact name within the team.
    pub bot: String,
    pub name: String,
    pub prompt: String,
    pub cadence: RoutineCadence,
    pub time: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekday: Option<RoutineWeekday>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub month_day: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_date: Option<String>,
    /// An IANA zone name. Unset or blank means the host's zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
}

/// Params of `team.routine_update`. Only the fields given change; an empty `zone` clears it to the host's zone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineUpdateParams {
    pub team: String,
    pub routine: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cadence: Option<RoutineCadence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekday: Option<RoutineWeekday>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub month_day: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
}

/// Selects one routine of a team by id or exact name: `team.routine_rm`, `team.routine_pause`, `team.routine_resume` and
/// `team.routine_run`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineRef {
    pub team: String,
    pub routine: String,
}

/// Params of `team.routine_runs`. The newest `limit` runs, 50 when unset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineRunsParams {
    pub team: String,
    pub routine: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

/// Result of `team.routine_list`, in creation order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineList {
    pub routines: Vec<Routine>,
}

/// One run of a routine: an occurrence it claimed, or a run made now. Its task is absent when none was made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineRun {
    pub id: String,
    pub routine_id: String,
    /// The occurrence the run stands for.
    pub scheduled_for_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// Why the run or its task did not start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The protocol code of `error`, such as `bot.not_found` or `team.routine_run_interrupted`. Absent from older hosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    pub created_at_ms: i64,
}

/// Result of `team.routine_runs`, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineRunList {
    pub runs: Vec<RoutineRun>,
}
