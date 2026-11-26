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

#[cfg(test)]
mod tests {
    use super::{WallClock, occurrences_between, wall_clock_in};
    use crate::schedule::{Cadence, Schedule, ScheduleError};

    fn ms(text: &str) -> i64 {
        text.parse::<jiff::Timestamp>()
            .expect("test instant parses")
            .as_millisecond()
    }

    fn daily(time: &str, zone: Option<&str>) -> Schedule {
        Schedule {
            cadence: Cadence::Daily,
            weekday: None,
            day_of_month: None,
            date: None,
            time: time.to_owned(),
            zone: zone.map(str::to_owned),
        }
    }

    fn clock(date: &str, time: &str) -> Option<WallClock> {
        Some(WallClock {
            date: date.to_owned(),
            time: time.to_owned(),
        })
    }

    #[test]
    fn a_window_lists_its_occurrences_from_its_start_up_to_but_not_including_its_end() {
        let compiled = daily("09:00", Some("UTC")).compile().expect("valid");
        let found = occurrences_between(
            &compiled,
            ms("2026-11-25T00:00:00Z"),
            ms("2026-11-28T00:00:00Z"),
            100,
        );
        assert_eq!(
            found,
            vec![
                ms("2026-11-25T09:00:00Z"),
                ms("2026-11-26T09:00:00Z"),
                ms("2026-11-27T09:00:00Z"),
            ]
        );
    }

    #[test]
    fn an_occurrence_exactly_at_the_start_of_a_window_is_listed_and_one_at_its_end_is_not() {
        let compiled = daily("09:00", Some("UTC")).compile().expect("valid");
        let found = occurrences_between(
            &compiled,
            ms("2026-11-25T09:00:00Z"),
            ms("2026-11-26T09:00:00Z"),
            100,
        );
        assert_eq!(found, vec![ms("2026-11-25T09:00:00Z")]);
    }

    #[test]
    fn an_empty_or_reversed_window_has_no_occurrences() {
        let compiled = daily("09:00", Some("UTC")).compile().expect("valid");
        let instant = ms("2026-11-25T09:00:00Z");
        assert!(occurrences_between(&compiled, instant, instant, 100).is_empty());
        assert!(occurrences_between(&compiled, instant + 1, instant, 100).is_empty());
    }

    #[test]
    fn the_limit_keeps_the_earliest_occurrences_of_a_window() {
        let compiled = Schedule {
            cadence: Cadence::Hourly,
            ..daily("00:15", Some("UTC"))
        }
        .compile()
        .expect("valid");
        let found = occurrences_between(
            &compiled,
            ms("2026-11-25T00:00:00Z"),
            ms("2026-11-26T00:00:00Z"),
            3,
        );
        assert_eq!(
            found,
            vec![
                ms("2026-11-25T00:15:00Z"),
                ms("2026-11-25T01:15:00Z"),
                ms("2026-11-25T02:15:00Z"),
            ]
        );
    }

    #[test]
    fn a_run_reads_its_wall_clock_on_its_own_zone() {
        // Madrid is UTC+1 in late November.
        let at = ms("2026-11-25T08:00:00Z");
        assert_eq!(
            wall_clock_in(Some("Europe/Madrid"), at),
            Ok(clock("2026-11-25", "09:00"))
        );
    }

    #[test]
    fn a_wall_clock_can_fall_on_the_next_local_day() {
        let at = ms("2026-11-25T23:30:00Z");
        assert_eq!(
            wall_clock_in(Some("Europe/Madrid"), at),
            Ok(clock("2026-11-26", "00:30"))
        );
    }

    #[test]
    fn a_daily_run_keeps_its_local_time_across_the_daylight_saving_change() {
        // Madrid leaves summer time on 2026-10-25: 09:00 is 07:00Z before the change and 08:00Z after it.
        let compiled = daily("09:00", Some("Europe/Madrid"))
            .compile()
            .expect("valid");
        let found = occurrences_between(
            &compiled,
            ms("2026-10-23T00:00:00Z"),
            ms("2026-10-28T00:00:00Z"),
            100,
        );
        assert_eq!(found.len(), 5);
        assert_eq!(found[0], ms("2026-10-23T07:00:00Z"));
        assert_eq!(found[4], ms("2026-10-27T08:00:00Z"));
        for at in found {
            assert_eq!(
                wall_clock_in(Some("Europe/Madrid"), at).map(|clock| clock.map(|clock| clock.time)),
                Ok(Some("09:00".to_owned()))
            );
        }
    }

    #[test]
    fn a_zone_that_does_not_exist_has_no_wall_clock() {
        assert_eq!(
            wall_clock_in(Some("Mars/Olympus"), 0),
            Err(ScheduleError::UnknownZone("Mars/Olympus".to_owned()))
        );
    }
}
