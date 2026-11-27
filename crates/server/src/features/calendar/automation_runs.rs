//! Automation runs on the Calendar: each enabled automation's upcoming runs from the shared schedule engine. Automations
//! keep no run history yet, so they have no done runs.

use massorca_core_store::Store;
use massorca_protocol::ErrorBody;
use massorca_protocol::methods::calendar::{CalendarKind, CalendarOccurrence, MAX_OCCURRENCES};

use crate::features::automations::upcoming_runs;
use crate::features::teams::store_error;

use super::window::{Window, placed};

/// The upcoming runs of every automation in `window`, in no particular order.
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
        for at_ms in instants {
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
                done: false,
            });
        }
    }
    Ok(runs)
}
