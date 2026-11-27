//! The hourly and weekdays presets, and a claim after an outage of an hourly schedule (decision 0123).

use massorca_core_schedule::{Cadence, ClaimDecision, Schedule, decide_claim};

fn ms(text: &str) -> i64 {
    text.parse::<jiff::Timestamp>()
        .expect("test instant parses")
        .as_millisecond()
}

fn schedule(cadence: Cadence, time: &str) -> Schedule {
    Schedule {
        cadence,
        weekday: None,
        day_of_month: None,
        date: None,
        time: time.to_owned(),
        zone: Some("UTC".to_owned()),
    }
}

#[test]
fn hourly_fires_at_the_minute_of_its_time_in_every_hour() {
    let hourly = schedule(Cadence::Hourly, "00:15").compile().expect("valid");
    assert_eq!(
        hourly.next_after(ms("2026-05-04T09:20:00Z")),
        Some(ms("2026-05-04T10:15:00Z"))
    );
    assert_eq!(
        hourly.latest_at_or_before(ms("2026-05-04T09:20:00Z")),
        Some(ms("2026-05-04T09:15:00Z"))
    );
}

#[test]
fn weekdays_fire_from_monday_to_friday_only() {
    // 2026-05-08 is a Friday and 2026-05-11 a Monday.
    let weekdays = schedule(Cadence::Weekdays, "09:00")
        .compile()
        .expect("valid");
    assert_eq!(
        weekdays.next_after(ms("2026-05-08T10:00:00Z")),
        Some(ms("2026-05-11T09:00:00Z"))
    );
    assert_eq!(
        weekdays.latest_at_or_before(ms("2026-05-10T10:00:00Z")),
        Some(ms("2026-05-08T09:00:00Z"))
    );
}

#[test]
fn a_claim_after_an_hourly_outage_runs_the_latest_hour_once_and_counts_the_missed_ones() {
    let hourly = schedule(Cadence::Hourly, "00:15").compile().expect("valid");
    let decision = decide_claim(
        &hourly,
        Some(ms("2026-05-04T09:15:00Z")),
        ms("2026-05-04T12:40:00Z"),
    );
    assert_eq!(
        decision,
        ClaimDecision::Due {
            scheduled_for: ms("2026-05-04T12:15:00Z"),
            next_run_at: Some(ms("2026-05-04T13:15:00Z")),
            skipped: 3,
        }
    );
}
