//! Routine runs on the Calendar: a routine's upcoming runs from the shared schedule engine, and its done runs from its run
//! history. A paused routine has no upcoming runs.

use massorca_core_schedule::occurrences_between;
use massorca_core_store::{Store, TeamRoutineRow};
use massorca_protocol::ErrorBody;
use massorca_protocol::methods::calendar::{CalendarKind, CalendarOccurrence, MAX_OCCURRENCES};

use crate::features::teams::routines::schedule_of;
use crate::features::teams::{invalid, store_error};

use super::window::{Window, placed};

/// The run history read per routine. The store has no window query, so a window older than a routine's newest runs
/// shows none of them; ten thousand runs is years of a daily routine.
const RUN_HISTORY_LIMIT: usize = 10_000;

/// The upcoming and done runs of every routine in `window`, in no particular order.
pub(super) fn read(
    store: &Store,
    window: &Window,
    host_zone: Option<&str>,
) -> Result<Vec<CalendarOccurrence>, ErrorBody> {
    let mut runs = Vec::new();
    for team in store.list_teams().map_err(store_error)? {
        for row in store.list_routines(&team.id).map_err(store_error)? {
            runs.extend(upcoming(&row, window, host_zone)?);
            if window.include_done {
                runs.extend(done(store, &row, window, host_zone)?);
            }
        }
    }
    Ok(runs)
}

fn upcoming(
    row: &TeamRoutineRow,
    window: &Window,
    host_zone: Option<&str>,
) -> Result<Vec<CalendarOccurrence>, ErrorBody> {
    if row.paused {
        return Ok(Vec::new());
    }
    let schedule = schedule_of(row)?
        .compile()
        .map_err(|error| invalid(error.to_string()))?;
    occurrences_between(
        &schedule,
        window.upcoming_from(),
        window.to_ms,
        MAX_OCCURRENCES + 1,
    )
    .into_iter()
    .map(|at_ms| occurrence(row, at_ms, false, host_zone))
    .collect()
}

/// The runs that finished in the window. A run whose outcome is not recorded yet is still going, not done.
fn done(
    store: &Store,
    row: &TeamRoutineRow,
    window: &Window,
    host_zone: Option<&str>,
) -> Result<Vec<CalendarOccurrence>, ErrorBody> {
    store
        .list_routine_runs(&row.id, RUN_HISTORY_LIMIT)
        .map_err(store_error)?
        .into_iter()
        .filter(|run| {
            run.settled_at.is_some() && (window.from_ms..window.to_ms).contains(&run.scheduled_for)
        })
        .map(|run| occurrence(row, run.scheduled_for, true, host_zone))
        .collect()
}

fn occurrence(
    row: &TeamRoutineRow,
    at_ms: i64,
    is_done: bool,
    host_zone: Option<&str>,
) -> Result<CalendarOccurrence, ErrorBody> {
    let clock = placed(row.zone.as_deref(), at_ms)?;
    Ok(CalendarOccurrence {
        kind: CalendarKind::Routine,
        source_id: row.id.clone(),
        name: row.name.clone(),
        team_id: Some(row.team_id.clone()),
        project_id: None,
        at_ms,
        local_date: clock.date,
        local_time: clock.time,
        zone: row.zone.clone().or_else(|| host_zone.map(str::to_owned)),
        done: is_done,
    })
}
