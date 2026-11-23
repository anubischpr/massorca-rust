//! A wall time that does not exist fires when the gap ends; one that happens twice fires once, at the earlier instant.

use massorca_core_schedule::{Cadence, CompiledSchedule, Schedule};

fn ms(text: &str) -> i64 {
    text.parse::<jiff::Timestamp>()
        .expect("test instant parses")
        .as_millisecond()
}

fn daily(time: &str, zone: &str) -> CompiledSchedule {
    Schedule {
        cadence: Cadence::Daily,
        weekday: None,
        day_of_month: None,
        date: None,
        time: time.to_owned(),
        zone: Some(zone.to_owned()),
    }
    .compile()
    .expect("schedule is valid")
}

#[test]
fn new_york_spring_gap_fires_at_three() {
    // 2026-03-08 02:00 EST jumps to 03:00 EDT; 02:30 does not exist.
    let schedule = daily("02:30", "America/New_York");
    let gap = schedule.next_after(ms("2026-03-07T12:00:00Z"));
    assert_eq!(gap, Some(ms("2026-03-08T07:00:00Z")), "03:00 EDT");
    assert_eq!(
        schedule.next_after(ms("2026-03-08T07:00:00Z")),
        Some(ms("2026-03-09T06:30:00Z")),
        "next day is back on 02:30 EDT"
    );
    assert_eq!(
        schedule.latest_at_or_before(ms("2026-03-08T12:00:00Z")),
        gap,
        "the backward search agrees"
    );
}

#[test]
fn new_york_fall_overlap_fires_once_at_the_earlier_instant() {
    // 2026-11-01 02:00 EDT falls back to 01:00 EST; 01:30 happens twice.
    let schedule = daily("01:30", "America/New_York");
    let first = schedule.next_after(ms("2026-10-31T12:00:00Z"));
    assert_eq!(first, Some(ms("2026-11-01T05:30:00Z")), "01:30 EDT");
    assert_eq!(
        schedule.next_after(ms("2026-11-01T05:30:00Z")),
        Some(ms("2026-11-02T06:30:00Z")),
        "the second 01:30 (06:30Z) is not an occurrence"
    );
    assert_eq!(
        schedule.latest_at_or_before(ms("2026-11-01T07:00:00Z")),
        first,
        "after the second 01:30 the latest occurrence is still the first"
    );
}

#[test]
fn madrid_spring_gap_fires_at_three() {
    // 2026-03-29 02:00 CET jumps to 03:00 CEST.
    let schedule = daily("02:30", "Europe/Madrid");
    assert_eq!(
        schedule.next_after(ms("2026-03-28T12:00:00Z")),
        Some(ms("2026-03-29T01:00:00Z")),
        "03:00 CEST"
    );
}

#[test]
fn madrid_fall_overlap_fires_once_at_the_earlier_instant() {
    // 2026-10-25 03:00 CEST falls back to 02:00 CET; 02:30 happens twice.
    let schedule = daily("02:30", "Europe/Madrid");
    assert_eq!(
        schedule.next_after(ms("2026-10-24T12:00:00Z")),
        Some(ms("2026-10-25T00:30:00Z")),
        "02:30 CEST"
    );
    assert_eq!(
        schedule.next_after(ms("2026-10-25T00:30:00Z")),
        Some(ms("2026-10-26T01:30:00Z")),
        "02:30 CET the next day, not the repeated 02:30 at 01:30Z"
    );
}

#[test]
fn a_time_outside_the_change_is_untouched() {
    let schedule = daily("09:00", "America/New_York");
    assert_eq!(
        schedule.next_after(ms("2026-03-08T00:00:00Z")),
        Some(ms("2026-03-08T13:00:00Z")),
        "09:00 EDT on the change day"
    );
}
