//! `calendar.occurrences`: the runs of routines and automations in one window, and the done routine runs in it
//! (capability `calendar/1`, decision 0124). Upcoming runs come from the shared schedule engine; each run carries its
//! instant and its wall clock on its own zone.

use serde::{Deserialize, Serialize};

/// Params: [`CalendarOccurrencesParams`]. Result: [`CalendarOccurrenceList`].
pub const OCCURRENCES: &str = "calendar.occurrences";

/// The longest window one call may cover, in days.
pub const MAX_WINDOW_DAYS: i64 = 62;

/// The runs one call returns at most. A window with more keeps the earliest and sets `truncated`.
pub const MAX_OCCURRENCES: usize = 2_000;

/// Which kind of schedule a run belongs to: a Bot Teams routine or a Workspaces automation. A value this build does not
/// know decodes as `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalendarKind {
    Routine,
    Automation,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Params of `calendar.occurrences`. The window runs from `from_ms`, inclusive, to `to_ms`, exclusive, and spans at most
/// [`MAX_WINDOW_DAYS`]. Without `kind` both kinds are listed. Done routine runs are listed only with `include_done`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarOccurrencesParams {
    /// UTC epoch milliseconds.
    pub from_ms: i64,
    /// UTC epoch milliseconds.
    pub to_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<CalendarKind>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub include_done: bool,
}

/// One run on the calendar: an upcoming run of a routine or an automation, or a done routine run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarOccurrence {
    pub kind: CalendarKind,
    /// The routine or automation the run belongs to.
    pub source_id: String,
    pub name: String,
    /// The team of a routine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    /// The project of an automation; absent once that project is removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// When the run happens or happened, UTC epoch milliseconds.
    pub at_ms: i64,
    /// The date of `at_ms` on `zone`, `YYYY-MM-DD`.
    pub local_date: String,
    /// The time of `at_ms` on `zone`, 24-hour `HH:MM`.
    pub local_time: String,
    /// The IANA zone the run is placed on: its own zone, or the host's when it has none and the host's has a name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    /// Whether the run is done. Upcoming runs are not; only a done routine run is.
    pub done: bool,
}

/// Result of `calendar.occurrences`: the runs in the window, earliest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarOccurrenceList {
    pub occurrences: Vec<CalendarOccurrence>,
    /// Set when the window held more than [`MAX_OCCURRENCES`] runs; the list keeps the earliest of them.
    #[serde(default)]
    pub truncated: bool,
}
