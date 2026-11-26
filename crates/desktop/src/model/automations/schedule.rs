//! What an automation's schedule reads as on the page, and when it next runs (decision 0123).

use massorca_protocol::methods::automations::{Automation, AutomationCadence};

use super::form::weekday_label;

/// The schedule in words, with its zone: "Weekdays at 08:00, Europe/Madrid".
pub fn schedule_summary(automation: &Automation) -> String {
    let when = match automation.cadence {
        AutomationCadence::Hourly => {
            let minute = automation.time.get(3..).unwrap_or_default();
            format!("Every hour at :{minute}")
        }
        AutomationCadence::Daily => format!("Every day at {}", automation.time),
        AutomationCadence::Weekdays => format!("Weekdays at {}", automation.time),
        AutomationCadence::Weekly => format!(
            "Every {} at {}",
            automation.weekday.map_or("day", weekday_label),
            automation.time
        ),
        AutomationCadence::Cron => {
            format!("Cron {}", automation.cron.as_deref().unwrap_or_default())
        }
        AutomationCadence::Unknown => "A schedule this version does not show".to_owned(),
    };
    format!("{when}, {}", zone_text(automation))
}

/// The zone the automation runs in: its own, or the host's zone it resolves to.
fn zone_text(automation: &Automation) -> String {
    match (&automation.zone, &automation.zone_used) {
        (Some(zone), _) => zone.clone(),
        (None, Some(used)) => format!("host zone ({used})"),
        (None, None) => "host zone".to_owned(),
    }
}

/// When the automation next starts, read at `now_ms`: "Off" while disabled, otherwise the wait until its next run.
pub fn next_run_text(automation: &Automation, now_ms: i64) -> String {
    if !automation.enabled {
        return "Off".to_owned();
    }
    match automation.next_run_at_ms {
        None => "No next run".to_owned(),
        Some(next) => {
            let wait_ms = (next - now_ms).max(0);
            let minutes = wait_ms / 60_000;
            if minutes == 0 {
                "Next run now".to_owned()
            } else if minutes < 60 {
                format!("Next run in {minutes} min")
            } else if minutes < 24 * 60 {
                format!("Next run in {} h", minutes / 60)
            } else {
                format!("Next run in {} days", minutes / (24 * 60))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use massorca_protocol::methods::automations::AutomationWeekday;

    fn automation(cadence: AutomationCadence) -> Automation {
        Automation {
            id: "a1".to_owned(),
            project_id: Some("p1".to_owned()),
            project_name: Some("app".to_owned()),
            name: "Morning PRs".to_owned(),
            prompt: "Summarise".to_owned(),
            agent: "claude".to_owned(),
            cadence,
            time: "08:00".to_owned(),
            weekday: None,
            cron: None,
            zone: Some("Europe/Madrid".to_owned()),
            zone_used: None,
            grace_minutes: 720,
            enabled: true,
            next_run_at_ms: Some(0),
            last_run_at_ms: None,
            created_at_ms: 0,
            updated_at_ms: 0,
        }
    }

    #[test]
    fn each_cadence_reads_as_its_schedule_with_its_zone() {
        assert_eq!(
            schedule_summary(&automation(AutomationCadence::Weekdays)),
            "Weekdays at 08:00, Europe/Madrid"
        );
        assert_eq!(
            schedule_summary(&automation(AutomationCadence::Daily)),
            "Every day at 08:00, Europe/Madrid"
        );
        let mut hourly = automation(AutomationCadence::Hourly);
        hourly.time = "00:15".to_owned();
        assert_eq!(
            schedule_summary(&hourly),
            "Every hour at :15, Europe/Madrid"
        );
        let mut weekly = automation(AutomationCadence::Weekly);
        weekly.weekday = Some(AutomationWeekday::Friday);
        assert_eq!(
            schedule_summary(&weekly),
            "Every Friday at 08:00, Europe/Madrid"
        );
        let mut cron = automation(AutomationCadence::Cron);
        cron.cron = Some("0 9 * * 1-5".to_owned());
        cron.time = String::new();
        assert_eq!(schedule_summary(&cron), "Cron 0 9 * * 1-5, Europe/Madrid");
    }

    #[test]
    fn a_host_zone_is_named_by_the_zone_it_resolves_to() {
        let mut host = automation(AutomationCadence::Daily);
        host.zone = None;
        host.zone_used = Some("UTC".to_owned());
        assert_eq!(
            schedule_summary(&host),
            "Every day at 08:00, host zone (UTC)"
        );
    }

    #[test]
    fn the_next_run_reads_as_the_wait_from_now_and_a_disabled_one_is_off() {
        let mut due = automation(AutomationCadence::Daily);
        due.next_run_at_ms = Some(30 * 60_000);
        assert_eq!(next_run_text(&due, 0), "Next run in 30 min");
        due.next_run_at_ms = Some(5 * 3_600_000);
        assert_eq!(next_run_text(&due, 0), "Next run in 5 h");
        due.next_run_at_ms = Some(3 * 24 * 3_600_000);
        assert_eq!(next_run_text(&due, 0), "Next run in 3 days");
        due.next_run_at_ms = Some(0);
        assert_eq!(next_run_text(&due, 1_000), "Next run now");
        due.enabled = false;
        assert_eq!(next_run_text(&due, 0), "Off");
    }
}
