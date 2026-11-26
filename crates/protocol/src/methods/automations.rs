//! `automation.*`: scheduled agent runs, defined, listed, shown, edited, enabled and removed (decision 0123, capability
//! `automations/1`). These methods store definitions only; a run is not started by them.

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
