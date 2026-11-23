//! The due-run claim: only the latest missed run, nothing made up after a pause.

use massorca_core_schedule::{
    Cadence, ClaimDecision, CompiledSchedule, MAX_COUNTED_SKIPS, Schedule, decide_claim,
    next_run_from,
};

fn ms(text: &str) -> i64 {
    text.parse::<jiff::Timestamp>()
        .expect("test instant parses")
        .as_millisecond()
}

fn compiled(cadence: Cadence, date: Option<&str>) -> CompiledSchedule {
    Schedule {
        cadence,
        weekday: None,
        day_of_month: None,
        date: date.map(str::to_owned),
        time: "09:00".to_owned(),
        zone: Some("UTC".to_owned()),
    }
    .compile()
    .expect("schedule is valid")
}

#[test]
fn paused_or_finished_is_never_due() {
    let daily = compiled(Cadence::Daily, None);
    assert_eq!(
        decide_claim(&daily, None, ms("2026-05-04T10:00:00Z")),
        ClaimDecision::NotDue
    );
}

#[test]
fn a_future_next_run_is_not_due() {
    let daily = compiled(Cadence::Daily, None);
    assert_eq!(
        decide_claim(
            &daily,
            Some(ms("2026-05-04T09:00:00Z")),
            ms("2026-05-04T08:59:59Z")
        ),
        ClaimDecision::NotDue
    );
}

#[test]
fn an_on_time_run_is_due_and_moves_to_the_next_occurrence() {
    let daily = compiled(Cadence::Daily, None);
    assert_eq!(
        decide_claim(
            &daily,
            Some(ms("2026-05-04T09:00:00Z")),
            ms("2026-05-04T09:00:00Z")
        ),
        ClaimDecision::Due {
            scheduled_for: ms("2026-05-04T09:00:00Z"),
            next_run_at: Some(ms("2026-05-05T09:00:00Z")),
            skipped: 0,
        }
    );
}

#[test]
fn only_the_latest_missed_run_counts() {
    let daily = compiled(Cadence::Daily, None);
    assert_eq!(
        decide_claim(
            &daily,
            Some(ms("2026-05-01T09:00:00Z")),
            ms("2026-05-04T10:00:00Z")
        ),
        ClaimDecision::Due {
            scheduled_for: ms("2026-05-04T09:00:00Z"),
            next_run_at: Some(ms("2026-05-05T09:00:00Z")),
            skipped: 3,
        },
        "May 1, 2 and 3 are skipped; May 4 runs"
    );
}

#[test]
fn the_skip_count_is_capped() {
    let daily = compiled(Cadence::Daily, None);
    let ClaimDecision::Due { skipped, .. } = decide_claim(
        &daily,
        Some(ms("2020-01-01T09:00:00Z")),
        ms("2026-05-04T10:00:00Z"),
    ) else {
        panic!("a run years overdue is due");
    };
    assert_eq!(skipped, MAX_COUNTED_SKIPS);
}

#[test]
fn a_one_time_run_finishes_after_its_claim() {
    let once = compiled(Cadence::Once, Some("2026-06-01"));
    assert_eq!(
        decide_claim(
            &once,
            Some(ms("2026-06-01T09:00:00Z")),
            ms("2026-06-03T00:00:00Z")
        ),
        ClaimDecision::Due {
            scheduled_for: ms("2026-06-01T09:00:00Z"),
            next_run_at: None,
            skipped: 0,
        }
    );
}

#[test]
fn a_stored_time_the_schedule_no_longer_has_still_runs_once() {
    // The store's next run predates an edit; the edited schedule has no occurrence since it.
    let daily = compiled(Cadence::Daily, None);
    assert_eq!(
        decide_claim(
            &daily,
            Some(ms("2026-05-04T09:30:00Z")),
            ms("2026-05-04T10:00:00Z")
        ),
        ClaimDecision::Due {
            scheduled_for: ms("2026-05-04T09:30:00Z"),
            next_run_at: Some(ms("2026-05-05T09:00:00Z")),
            skipped: 0,
        }
    );
}

#[test]
fn resuming_starts_from_now_without_making_up_runs() {
    let daily = compiled(Cadence::Daily, None);
    assert_eq!(
        next_run_from(&daily, ms("2026-05-10T12:00:00Z")),
        Some(ms("2026-05-11T09:00:00Z"))
    );
    let once = compiled(Cadence::Once, Some("2026-06-01"));
    assert_eq!(
        next_run_from(&once, ms("2026-07-01T00:00:00Z")),
        None,
        "a one-time run missed while paused does not come back"
    );
}
