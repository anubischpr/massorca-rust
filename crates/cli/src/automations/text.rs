//! The lines `massorca automations` prints (decision 0123): one line per change, a table for the list and the details of
//! one automation. The next run is shown in UTC, as the other commands show their dates.

use massorca_protocol::methods::automations::{Automation, AutomationCadence, AutomationWeekday};

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

/// One row per automation: name, schedule, state and id, tab separated.
pub fn automation_table(automations: &[Automation]) -> Vec<String> {
    automations
        .iter()
        .map(|automation| {
            format!(
                "{}\t{}\t{}\t{}",
                automation.name,
                schedule_text(automation),
                state_text(automation),
                automation.id
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
        format!("agent: {}", automation.agent),
        format!("schedule: {}", schedule_text(automation)),
        format!("state: {}", state_text(automation)),
        format!("grace: {} minutes", automation.grace_minutes),
        format!("prompt: {}", automation.prompt),
    ]
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
