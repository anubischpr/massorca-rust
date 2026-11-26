//! A routine's run history as its page lists it (AUTO-28): each run with its time, whether a task started, and why not.

use jiff::{Timestamp, tz::TimeZone};
use massorca_protocol::methods::teams::RoutineRun;

use super::types::{RoutineId, TeamId};

/// One run of a routine's history, as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunRow {
    /// The occurrence the run stands for, in the viewer's zone: "Tue 1 Dec, 09:00".
    pub when: String,
    /// "Task started" when the run made a task, else "No task started".
    pub status: &'static str,
    /// Why the run or its task did not start.
    pub detail: Option<String>,
}

/// What the history page shows for its routine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunsState {
    Loading,
    Loaded(Vec<RunRow>),
    Failed(String),
}

/// The routine whose run history is open, and what the service answered for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutineHistory {
    pub team: TeamId,
    pub routine: RoutineId,
    pub runs: RunsState,
}

/// The rows of a history, newest first as the service lists them.
pub fn run_rows(runs: &[RoutineRun], zone: &TimeZone) -> Vec<RunRow> {
    runs.iter()
        .map(|run| {
            let started = run.task_id.is_some();
            RunRow {
                when: local_time(run.scheduled_for_ms, zone),
                status: if started {
                    "Task started"
                } else {
                    "No task started"
                },
                detail: run.error.clone(),
            }
        })
        .collect()
}

/// A UTC instant in `zone` as a short day and time: "Tue 1 Dec, 09:00"; empty for an instant out of range.
pub fn local_time(ms: i64, zone: &TimeZone) -> String {
    match Timestamp::from_millisecond(ms) {
        Ok(instant) => instant
            .to_zoned_datetime(zone.clone())
            .strftime("%a %-d %b, %H:%M")
            .to_string(),
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(scheduled: &str, task: Option<&str>, error: Option<&str>) -> RoutineRun {
        RoutineRun {
            id: format!("run-{scheduled}"),
            scheduled_for_ms: scheduled.parse::<Timestamp>().unwrap().as_millisecond(),
            task_id: task.map(Into::into),
            error: error.map(Into::into),
            ..crate::service::fixtures::base_routine_run()
        }
    }

    #[test]
    fn a_run_with_a_task_reads_as_started_and_one_without_says_why_it_did_not() {
        let rows = run_rows(
            &[
                run("2026-12-01T09:00:00Z", Some("k9"), None),
                run(
                    "2026-11-30T09:00:00Z",
                    None,
                    Some("The bot is not installed."),
                ),
            ],
            &TimeZone::UTC,
        );
        assert_eq!(rows[0].status, "Task started");
        assert_eq!(rows[0].detail, None);
        assert_eq!(rows[1].status, "No task started");
        assert_eq!(rows[1].detail.as_deref(), Some("The bot is not installed."));
    }

    #[test]
    fn a_run_shows_its_occurrence_in_the_viewers_zone() {
        let at = "2026-12-01T09:00:00Z".parse::<Timestamp>().unwrap();
        let ms = at.as_millisecond();
        assert_eq!(local_time(ms, &TimeZone::UTC), "Tue 1 Dec, 09:00");
        let madrid = TimeZone::get("Europe/Madrid").expect("the bundled zone");
        assert_eq!(local_time(ms, &madrid), "Tue 1 Dec, 10:00");
    }
}
