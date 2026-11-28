//! `automation.*`: scheduled agent runs, defined, listed, shown, edited, enabled and removed (decision 0123, capability
//! `automations/1`), and their runs: run now and run history (decision 0125, capability `automation-runs/1`). Where a run
//! works is a destination (decision 0127, capability `automation-destinations/1`).

use serde::{Deserialize, Serialize};

/// Params: [`AutomationCreateParams`]. Result: [`Automation`].
pub const CREATE: &str = "automation.create";
/// Params: none. Result: [`AutomationList`], in creation order.
pub const LIST: &str = "automation.list";
/// Params: [`AutomationRef`]. Result: [`Automation`].
pub const SHOW: &str = "automation.show";
/// Params: [`AutomationUpdateParams`]. Result: [`Automation`].
pub const UPDATE: &str = "automation.update";
/// Params: [`AutomationRef`]. Result: [`Automation`], the removed automation.
pub const REMOVE: &str = "automation.rm";
/// Params: [`AutomationRef`]. Result: [`Automation`]; its next run is scheduled from now.
pub const ENABLE: &str = "automation.enable";
/// Params: [`AutomationRef`]. Result: [`Automation`]; it has no next run until enabled.
pub const DISABLE: &str = "automation.disable";

/// Params: [`AutomationRef`]. Result: [`AutomationRun`], the run it recorded (capability `automation-runs/1`).
pub const RUN_NOW: &str = "automation.run_now";
/// Params: [`AutomationRunsParams`]. Result: [`AutomationRunList`], newest first (capability `automation-runs/1`).
pub const RUNS: &str = "automation.runs";

/// Topic of `automation.changed` (decision 0125): a run of the automation it names started, ended or was recorded, so
/// its history and its last and next runs changed.
pub const TOPIC_AUTOMATION_CHANGED: &str = "automation.changed";

/// The runs `automation.runs` returns when the params set no limit.
pub const DEFAULT_RUNS_LIMIT: u32 = 20;
/// The most runs one `automation.runs` page returns; retention keeps 100 final runs per automation.
pub const MAX_RUNS_LIMIT: u32 = 100;

/// The minutes a missed run may be late and still run, when an automation sets no grace (decision 0123).
pub const DEFAULT_GRACE_MINUTES: i64 = 720;
/// The longest grace an automation may set, in minutes: one week.
pub const MAX_GRACE_MINUTES: i64 = 10_080;

/// How an automation repeats. A value this build does not know decodes as `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationCadence {
    /// Every hour, at the minute of `time`.
    Hourly,
    Daily,
    /// Monday to Friday, at `time`.
    Weekdays,
    /// On `weekday`, at `time`.
    Weekly,
    /// The five fields of `cron`.
    Cron,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// The day a weekly automation runs. A value this build does not know decodes as `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationWeekday {
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

/// Where an automation's runs work (decision 0127). A value this build does not know decodes as `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationDestinationKind {
    /// The project's own place: one worktree kept for the automation, or its folder's workspace (decision 0125).
    #[default]
    ProjectDefault,
    /// A new worktree for each run, titled with the automation's name, from its base branch when it has one.
    ProjectWorktreePerRun,
    /// One existing workspace, a worktree or a folder: every run goes there.
    ExistingWorkspace,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Where an automation's runs work. `workspace` is set for `existing_workspace`, `base_branch` may be set for
/// `project_worktree_per_run`; `workspace_name` is only filled in by the service, while the workspace exists.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AutomationDestination {
    pub kind: AutomationDestinationKind,
    /// The existing workspace's id, for `existing_workspace`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_name: Option<String>,
    /// The branch a new worktree per run starts from; absent is the project's base.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,
}

/// An automation: an agent run on a schedule, in a project. Timestamps are UTC epoch milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Automation {
    pub id: String,
    /// The project the runs work in; absent once that project is removed, and such an automation never starts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// The project's name, while the project exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    pub name: String,
    pub prompt: String,
    /// The agent kind id, such as `claude`.
    pub agent: String,
    pub cadence: AutomationCadence,
    /// Wall-clock `HH:MM`, 24-hour; for an hourly automation, its minute.
    pub time: String,
    /// The day a weekly automation runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekday: Option<AutomationWeekday>,
    /// The five-field expression of a cron automation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    /// An IANA zone name; absent means the host's zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    /// For an automation without `zone`, the zone its occurrences use on the host now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_used: Option<String>,
    /// Minutes a missed run may be late and still run; [`DEFAULT_GRACE_MINUTES`] when the automation sets none.
    pub grace_minutes: i64,
    /// A disabled automation has no next run.
    pub enabled: bool,
    /// Where its runs work; the project's own place when the automation sets none (decision 0127).
    #[serde(default)]
    pub destination: AutomationDestination,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_run_at_ms: Option<i64>,
    /// When its last run started; absent until runs are recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run_at_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// Params of `automation.create`. `project` selects a project by id, exact name or path. Blank or unset `zone` means the
/// host's zone. Unset `grace_minutes` means the default. A `creation_key` a client repeats returns the automation the
/// first create made, instead of a second one. `weekday` is kept only by a weekly cadence and `cron` only by a cron one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationCreateParams {
    pub project: String,
    pub name: String,
    pub prompt: String,
    /// The agent kind id, such as `claude`.
    pub agent: String,
    pub cadence: AutomationCadence,
    pub time: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekday: Option<AutomationWeekday>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grace_minutes: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creation_key: Option<String>,
    /// Where the runs work: `project_worktree_per_run` or `existing_workspace` (capability `automation-destinations/1`).
    /// Absent keeps the project's own place. An existing workspace belongs to `project`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<AutomationDestination>,
}

/// Params of `automation.update`. Only the fields given change. A blank `zone` returns the automation to the host's
/// zone; a schedule change recomputes the next run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationUpdateParams {
    /// The automation, by id or exact name.
    pub automation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cadence: Option<AutomationCadence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekday: Option<AutomationWeekday>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cron: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grace_minutes: Option<i64>,
    /// Replaces the destination when given (capability `automation-destinations/1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<AutomationDestination>,
}

/// Selects one automation by id or exact name: `automation.show`, `automation.rm`, `automation.enable` and
/// `automation.disable`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationRef {
    pub automation: String,
}

/// Result of `automation.list`: every automation, in creation order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationList {
    pub automations: Vec<Automation>,
}

/// Params of `automation.runs`: one automation's runs, newest first, `limit` at a time ([`DEFAULT_RUNS_LIMIT`] when
/// unset, at most [`MAX_RUNS_LIMIT`]), numbered below `before` when it is given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationRunsParams {
    /// The automation, by id or exact name.
    pub automation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<i64>,
}

/// Result of `automation.runs`. `next_before` is the cursor of the next page, absent on the last one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationRunList {
    pub runs: Vec<AutomationRun>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_before: Option<i64>,
}

/// What started a run. A value this build does not know decodes as `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationRunTrigger {
    Schedule,
    Manual,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Where a run stands (AUTO-04). `dispatching` and `dispatched` are in progress; the rest are final. A value this build
/// does not know decodes as `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationRunState {
    Pending,
    /// Claimed; its agent is being started.
    Dispatching,
    /// Its agent runs; it ends when the agent reports Done or its run ends.
    Dispatched,
    /// The agent reported Done; `outcome` says how it went.
    Completed,
    SkippedPrecheck,
    /// The occurrence was later than its grace, or an earlier one a later occurrence replaced. Never replayed.
    SkippedMissed,
    /// The run could not start here, such as an automation without a project.
    SkippedUnavailable,
    SkippedNeedsInteractiveAuth,
    /// The agent could not start, exited without reporting Done, or its session was lost.
    DispatchFailed,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

impl AutomationRunState {
    /// Whether the run is still going: claimed or with its agent running.
    pub fn in_progress(self) -> bool {
        matches!(self, Self::Pending | Self::Dispatching | Self::Dispatched)
    }
}

/// How a completed run's agent said it went. A value this build does not know decodes as `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationRunOutcome {
    Success,
    Failure,
    Cancelled,
    Interrupted,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Whether a run's token usage is known. A value this build does not know decodes as `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationUsageStatus {
    Known,
    Unavailable,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// A run's token usage, or why it is not known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationRunUsage {
    pub status: AutomationUsageStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<i64>,
}

/// One run of an automation. Timestamps are UTC epoch milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationRun {
    pub id: String,
    pub automation_id: String,
    /// The automation's run number, counting from 1; never reused.
    pub number: i64,
    pub trigger: AutomationRunTrigger,
    pub state: AutomationRunState,
    /// The occurrence the run stands for, or when a manual run was asked for.
    pub scheduled_for_ms: i64,
    /// How many occurrences the row stands for: more than one when missed occurrences are folded together.
    pub occurrences: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<AutomationRunOutcome>,
    /// The agent session of the run, once its agent started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// Why the run was skipped or failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The protocol code of `reason`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    /// The agent's last result text, its most recent characters when it was longer than the host keeps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    pub usage: AutomationRunUsage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at_ms: Option<i64>,
    pub created_at_ms: i64,
}
