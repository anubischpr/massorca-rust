//! The run table of one automation on the Automations page (decision 0125, artboard `ws-automations.dc.html`): the runs
//! the service last reported, and the words each row shows. Plain data with no GPUI.

use massorca_protocol::methods::automations::{
    AutomationRun, AutomationRunOutcome, AutomationRunState,
};

/// The run table open on the page: its automation and its runs as last read, or why they could not be read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunHistory {
    automation: String,
    runs: Option<Result<Vec<AutomationRun>, String>>,
}

impl RunHistory {
    pub fn new(automation: String) -> Self {
        Self {
            automation,
            runs: None,
        }
    }

    pub fn automation(&self) -> &str {
        &self.automation
    }

    /// The runs, newest first; empty while unread or unreadable.
    pub fn runs(&self) -> &[AutomationRun] {
        match &self.runs {
            Some(Ok(runs)) => runs,
            _ => &[],
        }
    }

    pub fn error(&self) -> Option<&str> {
        match &self.runs {
            Some(Err(message)) => Some(message),
            _ => None,
        }
    }

    pub fn is_read(&self) -> bool {
        self.runs.is_some()
    }

    pub fn set_runs(&mut self, runs: Result<Vec<AutomationRun>, String>) {
        self.runs = Some(runs);
    }

    /// Puts a run the service just answered with: replaced where it was, first when it is new.
    pub fn put(&mut self, run: AutomationRun) {
        let runs = self.runs.get_or_insert_with(|| Ok(Vec::new()));
        if let Ok(runs) = runs {
            match runs.iter().position(|known| known.id == run.id) {
                Some(index) => runs[index] = run,
                None => runs.insert(0, run),
            }
        }
    }
}

/// The status a run row shows, in the artboard's words.
pub fn run_status_text(run: &AutomationRun) -> &'static str {
    match run.state {
        AutomationRunState::Pending | AutomationRunState::Dispatching => "Starting",
        AutomationRunState::Dispatched => "Running",
        AutomationRunState::Completed => match run.outcome {
            Some(AutomationRunOutcome::Success) => "Done",
            Some(AutomationRunOutcome::Cancelled) => "Stopped",
            _ => "Failed",
        },
        AutomationRunState::SkippedMissed => "Missed",
        AutomationRunState::SkippedPrecheck
        | AutomationRunState::SkippedUnavailable
        | AutomationRunState::SkippedNeedsInteractiveAuth => "Not started",
        AutomationRunState::DispatchFailed => "Failed",
        AutomationRunState::Unknown => "Unknown",
    }
}

/// When the run started, or the occurrence it stands for when it never started, read at `now_ms`: "12 min ago".
pub fn run_started_text(run: &AutomationRun, now_ms: i64) -> String {
    let at = run.started_at_ms.unwrap_or(run.scheduled_for_ms);
    let minutes = (now_ms - at).max(0) / 60_000;
    if minutes == 0 {
        "Just now".to_owned()
    } else if minutes < 60 {
        format!("{minutes} min ago")
    } else if minutes < 24 * 60 {
        format!("{} h ago", minutes / 60)
    } else {
        format!("{} days ago", minutes / (24 * 60))
    }
}

/// How long the run's agent ran, such as `3m 12s`, once it started and ended.
pub fn run_duration_text(run: &AutomationRun) -> Option<String> {
    let started = run.started_at_ms?;
    let ended = run.ended_at_ms?;
    let seconds = (ended - started).max(0) / 1000;
    Some(match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m {:02}s", seconds / 60, seconds % 60),
        _ => format!("{}h {:02}m", seconds / 3600, (seconds % 3600) / 60),
    })
}

/// The detail a row shows: the result the agent gave, or why the run did not run, on one line.
pub fn run_detail_text(run: &AutomationRun) -> String {
    let text = run
        .result
        .as_deref()
        .or(run.reason.as_deref())
        .unwrap_or_default();
    let line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default();
    if run.occurrences > 1 {
        return format!("{} occurrences. {line}", run.occurrences);
    }
    line.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use massorca_protocol::methods::automations::{
        AutomationRunTrigger, AutomationRunUsage, AutomationUsageStatus,
    };

    fn run(id: &str, state: AutomationRunState) -> AutomationRun {
        AutomationRun {
            id: id.to_owned(),
            automation_id: "a1".to_owned(),
            number: 1,
            trigger: AutomationRunTrigger::Schedule,
            state,
            scheduled_for_ms: 0,
            occurrences: 1,
            outcome: None,
            session_id: None,
            workspace_id: None,
            reason: None,
            reason_code: None,
            result: None,
            usage: AutomationRunUsage {
                status: AutomationUsageStatus::Unavailable,
                reason: None,
                input_tokens: None,
                output_tokens: None,
            },
            started_at_ms: None,
            ended_at_ms: None,
            created_at_ms: 0,
        }
    }

    #[test]
    fn a_new_run_comes_first_and_a_known_one_is_replaced_in_place() {
        let mut history = RunHistory::new("a1".to_owned());
        history.set_runs(Ok(vec![run("old", AutomationRunState::Completed)]));
        history.put(run("new", AutomationRunState::Dispatched));
        history.put(run("new", AutomationRunState::DispatchFailed));
        let ids: Vec<&str> = history.runs().iter().map(|run| run.id.as_str()).collect();
        assert_eq!(ids, ["new", "old"]);
        assert_eq!(history.runs()[0].state, AutomationRunState::DispatchFailed);
    }

    #[test]
    fn a_completed_run_reads_done_or_failed_by_its_outcome() {
        let mut done = run("r", AutomationRunState::Completed);
        done.outcome = Some(AutomationRunOutcome::Success);
        assert_eq!(run_status_text(&done), "Done");
        done.outcome = Some(AutomationRunOutcome::Failure);
        assert_eq!(run_status_text(&done), "Failed");
        assert_eq!(
            run_status_text(&run("m", AutomationRunState::SkippedMissed)),
            "Missed"
        );
    }

    #[test]
    fn the_start_reads_as_a_time_ago_from_the_occurrence_when_the_run_never_started() {
        let mut missed = run("m", AutomationRunState::SkippedMissed);
        missed.scheduled_for_ms = 0;
        assert_eq!(run_started_text(&missed, 3 * 3_600_000), "3 h ago");
        missed.started_at_ms = Some(3 * 3_600_000 - 120_000);
        assert_eq!(run_started_text(&missed, 3 * 3_600_000), "2 min ago");
    }

    #[test]
    fn the_duration_reads_in_minutes_and_seconds() {
        let mut ended = run("r", AutomationRunState::Completed);
        ended.started_at_ms = Some(1_000);
        ended.ended_at_ms = Some(1_000 + 192_000);
        assert_eq!(run_duration_text(&ended).as_deref(), Some("3m 12s"));
        ended.ended_at_ms = None;
        assert_eq!(run_duration_text(&ended), None);
    }

    #[test]
    fn the_detail_is_the_first_line_of_the_result_or_the_reason() {
        let mut missed = run("m", AutomationRunState::SkippedMissed);
        missed.reason = Some("Later than its grace.".to_owned());
        missed.occurrences = 3;
        assert_eq!(
            run_detail_text(&missed),
            "3 occurrences. Later than its grace."
        );
        let mut done = run("d", AutomationRunState::Completed);
        done.result = Some("\nAll green.\nMore".to_owned());
        assert_eq!(run_detail_text(&done), "All green.");
    }
}
