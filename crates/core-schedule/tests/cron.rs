//! Cron expressions: validation, the days and times they name, and their occurrences (decision 0123).

use massorca_core_schedule::{CronSchedule, MAX_CRON_EXPRESSION_LEN, ScheduleError};

fn ms(text: &str) -> i64 {
    text.parse::<jiff::Timestamp>()
        .expect("test instant parses")
        .as_millisecond()
}

fn cron(expression: &str, zone: Option<&str>) -> CronSchedule {
    CronSchedule {
        expression: expression.to_owned(),
        zone: zone.map(str::to_owned),
    }
}

fn refusal(expression: &str) -> ScheduleError {
    cron(expression, Some("UTC"))
        .compile()
        .expect_err("the expression is refused")
}

fn next(expression: &str, zone: Option<&str>, after: &str) -> Option<i64> {
    cron(expression, zone)
        .compile()
        .expect("the expression is valid")
        .next_after(ms(after))
}

#[test]
fn a_cron_expression_has_exactly_five_fields() {
    assert_eq!(refusal("0 9 * *"), ScheduleError::CronFieldCount(4));
    assert_eq!(refusal("0 0 9 * * *"), ScheduleError::CronFieldCount(6));
}

#[test]
fn a_field_outside_its_range_or_with_an_unknown_token_is_refused_by_name() {
    assert_eq!(
        refusal("0 24 * * *"),
        ScheduleError::CronField {
            field: "hour",
            text: "24".to_owned()
        }
    );
    assert_eq!(
        refusal("60 9 * * *"),
        ScheduleError::CronField {
            field: "minute",
            text: "60".to_owned()
        }
    );
    assert_eq!(
        refusal("0 9 0 * *"),
        ScheduleError::CronField {
            field: "day of month",
            text: "0".to_owned()
        }
    );
    assert_eq!(
        refusal("0 9 * 13 *"),
        ScheduleError::CronField {
            field: "month",
            text: "13".to_owned()
        }
    );
    assert_eq!(
        refusal("0 9 * * 8"),
        ScheduleError::CronField {
            field: "day of week",
            text: "8".to_owned()
        }
    );
    assert_eq!(
        refusal("0 9 * * MONDAYS"),
        ScheduleError::CronField {
            field: "day of week",
            text: "MONDAYS".to_owned()
        }
    );
}

#[test]
fn a_step_larger_than_its_field_is_refused() {
    assert_eq!(
        refusal("*/60 * * * *"),
        ScheduleError::CronStepTooLarge {
            field: "minute",
            step: 60
        }
    );
}

#[test]
fn an_expression_that_can_never_occur_is_refused() {
    for expression in ["0 0 30 2 *", "0 0 31 2 *", "0 0 31 4,6,9,11 *"] {
        assert_eq!(
            refusal(expression),
            ScheduleError::CronNeverOccurs,
            "{expression}"
        );
    }
}

#[test]
fn a_leap_day_is_accepted_and_found_across_the_century_gap() {
    assert_eq!(
        next("0 0 29 2 *", Some("UTC"), "2026-03-01T00:00:00Z"),
        Some(ms("2028-02-29T00:00:00Z"))
    );
    // 2100 is not a leap year, so the gap from 2096 to 2104 is eight years.
    assert_eq!(
        next("0 0 29 2 *", Some("UTC"), "2096-03-01T00:00:00Z"),
        Some(ms("2104-02-29T00:00:00Z"))
    );
}

#[test]
fn an_impossible_day_of_month_still_occurs_when_the_weekday_alternative_matches() {
    // Day 31 of February never exists, but the weekday alternative does: Mondays in February.
    assert_eq!(
        next("0 9 31 2 1", Some("UTC"), "2026-02-01T00:00:00Z"),
        Some(ms("2026-02-02T09:00:00Z"))
    );
}

#[test]
fn sunday_is_zero_or_seven_and_names_are_accepted() {
    // 2026-05-04 is a Monday.
    assert_eq!(
        next("0 9 * * 7", Some("UTC"), "2026-05-04T09:00:00Z"),
        Some(ms("2026-05-10T09:00:00Z"))
    );
    assert_eq!(
        next("0 9 * * SUN", Some("UTC"), "2026-05-04T09:00:00Z"),
        Some(ms("2026-05-10T09:00:00Z"))
    );
    assert_eq!(
        next("0 9 * JAN-MAR MON", Some("UTC"), "2026-03-30T10:00:00Z"),
        Some(ms("2027-01-04T09:00:00Z"))
    );
}

#[test]
fn lists_ranges_and_steps_name_every_matching_time_in_order() {
    let schedule = cron("0,30 9-10 * * *", Some("UTC"))
        .compile()
        .expect("valid");
    assert_eq!(
        schedule.next_after(ms("2026-05-04T09:10:00Z")),
        Some(ms("2026-05-04T09:30:00Z"))
    );
    assert_eq!(
        schedule.next_after(ms("2026-05-04T09:30:00Z")),
        Some(ms("2026-05-04T10:00:00Z"))
    );
    assert_eq!(
        next("*/15 * * * *", Some("UTC"), "2026-05-04T09:07:00Z"),
        Some(ms("2026-05-04T09:15:00Z"))
    );
}

#[test]
fn a_restricted_day_of_month_and_day_of_week_match_either_when_both_are_given() {
    // The 1st of the month or any Monday, as standard cron reads it. 2026-05-01 is a Friday.
    assert_eq!(
        next("0 9 1 * 1", Some("UTC"), "2026-05-02T10:00:00Z"),
        Some(ms("2026-05-04T09:00:00Z"))
    );
}

#[test]
fn a_star_day_of_month_with_a_weekday_matches_only_that_weekday() {
    assert_eq!(
        next("0 9 * * 1", Some("UTC"), "2026-05-04T09:00:00Z"),
        Some(ms("2026-05-11T09:00:00Z"))
    );
}

#[test]
fn an_expression_longer_than_the_limit_is_refused_before_it_is_parsed() {
    let long = format!("{}0 9 * * *", " ".repeat(MAX_CRON_EXPRESSION_LEN));
    assert_eq!(refusal(&long), ScheduleError::CronTooLong(long.len()));
}

#[test]
fn an_unknown_zone_is_refused() {
    let error = cron("0 9 * * *", Some("Mars/Olympus"))
        .compile()
        .expect_err("unknown zone");
    assert_eq!(error, ScheduleError::UnknownZone("Mars/Olympus".to_owned()));
}

#[test]
fn a_wall_time_skipped_by_spring_forward_fires_at_the_end_of_the_gap() {
    // Madrid jumps from 02:00 to 03:00 on 2026-03-29, so 02:30 does not exist that day.
    assert_eq!(
        next("30 2 * * *", Some("Europe/Madrid"), "2026-03-29T00:30:00Z"),
        Some(ms("2026-03-29T01:00:00Z"))
    );
    assert_eq!(
        next("30 2 * * *", Some("Europe/Madrid"), "2026-03-29T01:00:00Z"),
        Some(ms("2026-03-30T00:30:00Z"))
    );
}

#[test]
fn a_wall_time_repeated_by_fall_back_fires_once_at_the_earlier_instant() {
    // Madrid repeats 02:00 to 03:00 on 2026-10-25, so 02:30 happens twice and fires once.
    assert_eq!(
        next("30 2 * * *", Some("Europe/Madrid"), "2026-10-25T00:00:00Z"),
        Some(ms("2026-10-25T00:30:00Z"))
    );
    assert_eq!(
        next("30 2 * * *", Some("Europe/Madrid"), "2026-10-25T00:30:00Z"),
        Some(ms("2026-10-26T01:30:00Z"))
    );
}
