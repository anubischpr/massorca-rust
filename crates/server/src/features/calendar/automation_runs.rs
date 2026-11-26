//! Automation runs on the Calendar: each enabled automation's upcoming runs from the shared schedule engine, and its
//! completed runs from its run history (decision 0125).

use massorca_core_store::Store;
use massorca_protocol::ErrorBody;
use massorca_protocol::methods::calendar::{CalendarKind, CalendarOccurrence, MAX_OCCURRENCES};

use crate::features::automations::{done_runs, upcoming_runs};
use crate::features::teams::store_error;

use super::window::{Window, placed};

/// The upcoming and completed runs of every automation in `window`, in no particular order.
pub(super) fn read(
    store: &Store,
    window: &Window,
    host_zone: Option<&str>,
) -> Result<Vec<CalendarOccurrence>, ErrorBody> {
    let mut runs = Vec::new();
    for row in store.list_automations().map_err(store_error)? {
        let (automation, instants) = upcoming_runs(
            store,
            &row,
            window.upcoming_from(),
            window.to_ms,
            MAX_OCCURRENCES + 1,
        )?;
        let done = if window.include_done {
            done_runs(store, &automation.id, window.from_ms, window.to_ms)?
        } else {
            Vec::new()
        };
        let placed_runs = instants
            .into_iter()
            .map(|at_ms| (at_ms, false))
            .chain(done.into_iter().map(|at_ms| (at_ms, true)));
        for (at_ms, done) in placed_runs {
            let clock = placed(automation.zone.as_deref(), at_ms)?;
            runs.push(CalendarOccurrence {
                kind: CalendarKind::Automation,
                source_id: automation.id.clone(),
                name: automation.name.clone(),
                team_id: None,
                project_id: automation.project_id.clone(),
                at_ms,
                local_date: clock.date,
                local_time: clock.time,
                zone: automation
                    .zone
                    .clone()
                    .or_else(|| host_zone.map(str::to_owned)),
                done,
            });
        }
    }
    Ok(runs)
}

#[cfg(test)]
mod tests {
    use massorca_core_store::{AutomationRow, AutomationRunRow, Store, new_id, run_state};

    use super::read;
    use crate::features::calendar::window::Window;

    const DAY: i64 = 86_400_000;
    /// 2026-11-25 09:00 UTC.
    const RAN_AT: i64 = 1_795_597_200_000;

    fn store_with_runs() -> Store {
        let store = Store::open_in_memory().expect("in-memory store opens");
        let automation = AutomationRow {
            id: "auto".to_owned(),
            project_id: None,
            name: "Morning PRs".to_owned(),
            prompt: "Summarise".to_owned(),
            agent_kind: "claude".to_owned(),
            cadence: "daily".to_owned(),
            time: "09:00".to_owned(),
            weekday: None,
            cron: None,
            zone: Some("UTC".to_owned()),
            grace_minutes: None,
            enabled: false,
            creation_key: None,
            next_run_at: None,
            last_run_at: None,
            destination: "project_default".to_owned(),
            destination_workspace_id: None,
            destination_path: None,
            base_branch: None,
            created_at: 1,
            updated_at: 1,
        };
        store.insert_automation(&automation).unwrap();
        for (state, at) in [
            (run_state::COMPLETED, RAN_AT),
            (run_state::DISPATCH_FAILED, RAN_AT + DAY),
        ] {
            let mut run = AutomationRunRow {
                id: new_id(),
                automation_id: "auto".to_owned(),
                trigger: "schedule".to_owned(),
                scheduled_for: at,
                occurrences: 1,
                state: state.to_owned(),
                created_at: at,
                ..AutomationRunRow::default()
            };
            store.insert_automation_run(&mut run, None).unwrap();
        }
        store
    }

    fn window(include_done: bool) -> Window {
        Window {
            from_ms: RAN_AT - DAY,
            to_ms: RAN_AT + 3 * DAY,
            now_ms: RAN_AT + 3 * DAY,
            include_done,
        }
    }

    #[test]
    fn a_completed_automation_run_is_done_only_when_asked_for_and_a_failed_one_never_is() {
        let store = store_with_runs();

        assert!(read(&store, &window(false), None).unwrap().is_empty());
        let done = read(&store, &window(true), None).unwrap();

        assert_eq!(done.len(), 1);
        assert!(done[0].done);
        assert_eq!(done[0].at_ms, RAN_AT);
        assert_eq!(done[0].local_time, "09:00");
    }
}
