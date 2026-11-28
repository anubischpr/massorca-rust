//! The upcoming runs of an automation for the Calendar: the shared schedule engine's occurrences, read through the
//! automation view. A disabled automation has none.

use massorca_core_schedule::occurrences_between;
use massorca_core_store::{AutomationRow, Store, run_state};
use massorca_protocol::ErrorBody;
use massorca_protocol::methods::automations::Automation;

use super::definition::{HostZone, compile};
use super::runs::KEPT_FINAL_RUNS;
use super::view::automation_view;
use crate::features::teams::store_error;

/// The instants of the automation's completed runs whose occurrence falls in `[from_ms, to_ms)`, from its kept history.
pub(crate) fn done_runs(
    store: &Store,
    automation_id: &str,
    from_ms: i64,
    to_ms: i64,
) -> Result<Vec<i64>, ErrorBody> {
    Ok(store
        .list_automation_runs(automation_id, KEPT_FINAL_RUNS * 2, None)
        .map_err(store_error)?
        .into_iter()
        .filter(|run| {
            run.state == run_state::COMPLETED && (from_ms..to_ms).contains(&run.scheduled_for)
        })
        .map(|run| run.scheduled_for)
        .collect())
}

/// The automation as the protocol shows it, with the instants of its runs in `[from_ms, to_ms)`, earliest first and at
/// most `limit` of them.
pub(crate) fn upcoming_runs(
    store: &Store,
    row: &AutomationRow,
    from_ms: i64,
    to_ms: i64,
    limit: usize,
) -> Result<(Automation, Vec<i64>), ErrorBody> {
    let view = automation_view(store, row, &HostZone::read())?;
    if !view.enabled {
        return Ok((view, Vec::new()));
    }
    let schedule = compile(row)?;
    let instants = occurrences_between(&schedule, from_ms, to_ms, limit);
    Ok((view, instants))
}
