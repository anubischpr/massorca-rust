//! The lines `massorca automations` prints (decision 0123): one line per change, a table for the list and the details of
//! one automation. The next run is shown in UTC, as the other commands show their dates.

use massorca_protocol::methods::automations::{
    Automation, AutomationCadence, AutomationDestination, AutomationDestinationKind, AutomationRun,
    AutomationRunList, AutomationRunState, AutomationWeekday,
};

use crate::time_text::utc_minute;

/// One line for a changed automation: its verb, name, id and project.
pub fn automation_line(automation: &Automation, verb: &str) -> String {
    format!(
        "{verb} automation {} ({}) in {}",
        automation.name,
        automation.id,
        project_text(automation)
    )
}

/// One row per automation: name, schedule, state, id and destination, tab separated.
pub fn automation_table(automations: &[Automation]) -> Vec<String> {
    automations
        .iter()
        .map(|automation| {
            format!(
                "{}\t{}\t{}\t{}\t{}",
                automation.name,
                schedule_text(automation),
                state_text(automation),
                automation.id,
                destination_text(&automation.destination)
            )
        })
        .collect()
}

/// The fields of one automation, one per line.
pub fn automation_details(automation: &Automation) -> Vec<String> {
    vec![
        format!("name: {}", automation.name),
        format!("id: {}", automation.id),
        format!("project: {}", project_text(automation)),
        format!("destination: {}", destination_text(&automation.destination)),
        format!("agent: {}", automation.agent),
        format!("schedule: {}", schedule_text(automation)),
        format!("state: {}", state_text(automation)),
        format!("grace: {} minutes", automation.grace_minutes),
        format!("prompt: {}", automation.prompt),
    ]
}

/// Where the runs work: the project's own place, a new worktree per run, or the existing workspace, by name when it
/// still exists.
pub fn destination_text(destination: &AutomationDestination) -> String {
    match destination.kind {
        AutomationDestinationKind::ProjectDefault => "the project's own place".to_owned(),
        AutomationDestinationKind::ProjectWorktreePerRun => match &destination.base_branch {
            Some(branch) => format!("a new worktree per run from {branch}"),
            None => "a new worktree per run".to_owned(),
        },
        AutomationDestinationKind::ExistingWorkspace => {
            match (&destination.workspace_name, &destination.workspace) {
                (Some(name), Some(id)) => format!("workspace {name} ({id})"),
                (None, Some(id)) => format!("workspace {id} (no longer exists)"),
                _ => "workspace (not set)".to_owned(),
            }
        }
        AutomationDestinationKind::Unknown => "a destination this version does not know".to_owned(),
    }
}

fn project_text(automation: &Automation) -> String {
    match &automation.project_name {
        Some(name) => name.clone(),
        None => "no project (the project was removed)".to_owned(),
    }
}

fn schedule_text(automation: &Automation) -> String {
    let time = automation.time.as_str();
    let when = match automation.cadence {
        AutomationCadence::Hourly => {
            format!("hourly at :{}", time.get(3..).unwrap_or_default())
        }
        AutomationCadence::Daily => format!("daily at {time}"),
        AutomationCadence::Weekdays => format!("weekdays at {time}"),
        AutomationCadence::Weekly => {
            format!("weekly {} at {time}", weekday_word(automation.weekday))
        }
        AutomationCadence::Cron => {
            format!("cron {}", automation.cron.as_deref().unwrap_or_default())
        }
        AutomationCadence::Unknown => "unknown cadence".to_owned(),
    };
    let zone = match (&automation.zone, &automation.zone_used) {
        (Some(zone), _) => zone.clone(),
        (None, Some(used)) => format!("host zone ({used})"),
        (None, None) => "host zone".to_owned(),
    };
    format!("{when} {zone}")
}

fn state_text(automation: &Automation) -> String {
    if !automation.enabled {
        return "disabled".to_owned();
    }
    match automation.next_run_at_ms {
        Some(next) => format!("next run {}", utc_minute(next)),
        None => "no next run".to_owned(),
    }
}

fn weekday_word(day: Option<AutomationWeekday>) -> &'static str {
    match day {
        Some(AutomationWeekday::Monday) => "monday",
        Some(AutomationWeekday::Tuesday) => "tuesday",
        Some(AutomationWeekday::Wednesday) => "wednesday",
        Some(AutomationWeekday::Thursday) => "thursday",
        Some(AutomationWeekday::Friday) => "friday",
        Some(AutomationWeekday::Saturday) => "saturday",
        Some(AutomationWeekday::Sunday) => "sunday",
        Some(AutomationWeekday::Unknown) | None => "day ?",
    }
}

/// One line for a run: its number, state, when it stands for, and why it did not run when it did not.
pub fn run_line(run: &AutomationRun) -> String {
    let mut line = format!(
        "run {} {} for {}",
        run.number,
        run_state_text(run.state),
        utc_minute(run.scheduled_for_ms)
    );
    if run.occurrences > 1 {
        line.push_str(&format!(" ({} occurrences)", run.occurrences));
    }
    if let Some(reason) = &run.reason {
        line.push_str(&format!(": {reason}"));
    }
    line
}

/// One row per run, newest first, then the cursor of the next page when there is one.
pub fn run_table(list: &AutomationRunList) -> Vec<String> {
    if list.runs.is_empty() {
        return vec!["no runs".to_owned()];
    }
    let mut lines: Vec<String> = list.runs.iter().map(run_line).collect();
    if let Some(before) = list.next_before {
        lines.push(format!("more: --before {before}"));
    }
    lines
}

fn run_state_text(state: AutomationRunState) -> &'static str {
    match state {
        AutomationRunState::Pending => "pending",
        AutomationRunState::Dispatching => "starting",
        AutomationRunState::Dispatched => "running",
        AutomationRunState::Completed => "completed",
        AutomationRunState::SkippedPrecheck => "skipped (precheck)",
        AutomationRunState::SkippedMissed => "missed",
        AutomationRunState::SkippedUnavailable => "not started",
        AutomationRunState::SkippedNeedsInteractiveAuth => "skipped (needs sign-in)",
        AutomationRunState::DispatchFailed => "failed",
        AutomationRunState::Unknown => "unknown",
    }
}
