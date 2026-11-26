//! Occurrences inside a window of instants, and the wall clock an instant reads on its zone.

use jiff::Timestamp;

use crate::occurrence::CompiledSchedule;
use crate::schedule::{ScheduleError, resolve_zone};

/// The wall clock of an instant on a zone: a `YYYY-MM-DD` date and a 24-hour `HH:MM` time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallClock {
    pub date: String,
    pub time: String,
}

/// The occurrences of `schedule` at or after `from_ms` and before `to_ms`, earliest first, at most `limit` of them.
/// Instants are UTC epoch milliseconds; an empty or reversed window has none.
pub fn occurrences_between(
    schedule: &CompiledSchedule,
    from_ms: i64,
    to_ms: i64,
    limit: usize,
) -> Vec<i64> {
    let mut found = Vec::new();
    // `next_after` is strictly later than its argument, so the search starts one millisecond before the window.
    let Some(mut cursor) = from_ms.checked_sub(1) else {
        return found;
    };
    while found.len() < limit {
        let Some(next) = schedule.next_after(cursor) else {
            break;
        };
        if next >= to_ms {
            break;
        }
        found.push(next);
        cursor = next;
    }
    found
}

/// The wall clock `at_ms` reads on `zone`, an IANA name, or the host's zone when `None`. `Ok(None)` when the instant
/// is outside the representable range.
pub fn wall_clock_in(zone: Option<&str>, at_ms: i64) -> Result<Option<WallClock>, ScheduleError> {
    let (zone, _) = resolve_zone(zone)?;
    let Ok(at) = Timestamp::from_millisecond(at_ms) else {
        return Ok(None);
    };
    let civil = zone.to_datetime(at);
    Ok(Some(WallClock {
        date: civil.date().to_string(),
        time: format!("{:02}:{:02}", civil.hour(), civil.minute()),
    }))
}
