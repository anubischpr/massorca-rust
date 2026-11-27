//! An automation as the protocol shows it: its project's name while the project exists, and the zone it runs in.

use massorca_core_store::{AutomationRow, Store, now_ms};
use massorca_protocol::ErrorBody;
use massorca_protocol::methods::automations::{self, Automation};

use super::definition::{HostZone, cadence_of, compile, next_run, weekday_of};
use crate::features::teams::store_error;

pub(super) fn automation_view(
    store: &Store,
    row: &AutomationRow,
    host: &HostZone,
) -> Result<Automation, ErrorBody> {
    let project_name = match row.project_id.as_deref() {
        Some(id) => store
            .get_project(id)
            .map_err(store_error)?
            .map(|project| project.name),
        None => None,
    };
    Ok(Automation {
        id: row.id.clone(),
        project_id: row.project_id.clone(),
        project_name,
        name: row.name.clone(),
        prompt: row.prompt.clone(),
        agent: row.agent_kind.clone(),
        cadence: cadence_of(&row.cadence),
        time: row.time.clone(),
        weekday: row.weekday.as_deref().map(weekday_of),
        cron: row.cron.clone(),
        zone: row.zone.clone(),
        zone_used: row.zone.is_none().then(|| host.label()),
        grace_minutes: row
            .grace_minutes
            .unwrap_or(automations::DEFAULT_GRACE_MINUTES),
        enabled: row.enabled,
        next_run_at_ms: shown_next_run(row, now_ms()),
        last_run_at_ms: row.last_run_at,
        created_at_ms: row.created_at,
        updated_at_ms: row.updated_at,
    })
}

/// The next run from `now`. Runs do not advance the stored next run yet, so a read computes it from the clock.
fn shown_next_run(row: &AutomationRow, now: i64) -> Option<i64> {
    let compiled = compile(row).ok()?;
    next_run(row, &compiled, now)
}

#[cfg(test)]
mod tests {
    use massorca_core_store::AutomationRow;

    use super::shown_next_run;

    const HOUR_MS: i64 = 3_600_000;
    const DAY_MS: i64 = 24 * HOUR_MS;
    /// 2026-11-25 00:00 UTC.
    const NOV_25: i64 = 1_795_564_800_000;

    /// A daily automation in UTC at `time`; its stored next run is 09:00 on 25 November.
    fn daily_at(time: &str, enabled: bool) -> AutomationRow {
        AutomationRow {
            id: "a1".to_owned(),
            project_id: None,
            name: "Morning PRs".to_owned(),
            prompt: "Summarise the open pull requests".to_owned(),
            agent_kind: "claude".to_owned(),
            cadence: "daily".to_owned(),
            time: time.to_owned(),
            weekday: None,
            cron: None,
            zone: Some("UTC".to_owned()),
            grace_minutes: None,
            enabled,
            creation_key: None,
            next_run_at: Some(NOV_25 + 9 * HOUR_MS),
            last_run_at: None,
            created_at: NOV_25,
            updated_at: NOV_25,
        }
    }

    #[test]
    fn a_daily_run_read_after_its_time_shows_the_next_day() {
        let row = daily_at("09:00", true);
        let read_at = NOV_25 + 10 * HOUR_MS;
        assert_eq!(
            shown_next_run(&row, read_at),
            Some(NOV_25 + DAY_MS + 9 * HOUR_MS)
        );
    }

    #[test]
    fn a_daily_run_read_before_its_time_shows_that_day() {
        let row = daily_at("09:00", true);
        assert_eq!(
            shown_next_run(&row, NOV_25 + 8 * HOUR_MS),
            Some(NOV_25 + 9 * HOUR_MS)
        );
    }

    #[test]
    fn a_disabled_automation_shows_no_next_run() {
        assert_eq!(shown_next_run(&daily_at("09:00", false), NOV_25), None);
    }
}
