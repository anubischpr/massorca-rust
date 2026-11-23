//! Cadences, validation and bounded searches.

use massorca_core_schedule::{Cadence, Schedule, ScheduleError, Weekday, ZoneSource};

fn ms(text: &str) -> i64 {
    text.parse::<jiff::Timestamp>()
        .expect("test instant parses")
        .as_millisecond()
}

fn schedule(cadence: Cadence, time: &str, zone: Option<&str>) -> Schedule {
    Schedule {
        cadence,
        weekday: None,
        day_of_month: None,
        date: None,
        time: time.to_owned(),
        zone: zone.map(str::to_owned),
    }
}

fn next(schedule: &Schedule, after: &str) -> Option<i64> {
    schedule
        .compile()
        .expect("schedule is valid")
        .next_after(ms(after))
}

#[test]
fn daily_fires_later_the_same_day_then_the_next_day() {
    let daily = schedule(Cadence::Daily, "09:00", Some("UTC"));
    assert_eq!(
        next(&daily, "2026-05-04T08:00:00Z"),
        Some(ms("2026-05-04T09:00:00Z"))
    );
    assert_eq!(
        next(&daily, "2026-05-04T09:00:00Z"),
        Some(ms("2026-05-05T09:00:00Z")),
        "strictly after"
    );
}

#[test]
fn daily_uses_the_named_zone() {
    let daily = schedule(Cadence::Daily, "09:00", Some("Europe/Madrid"));
    assert_eq!(
        next(&daily, "2026-05-04T00:00:00Z"),
        Some(ms("2026-05-04T07:00:00Z"))
    );
}

#[test]
fn weekly_fires_on_its_weekday() {
    let mut weekly = schedule(Cadence::Weekly, "18:30", Some("UTC"));
    weekly.weekday = Some(Weekday::Friday);
    // 2026-05-04 is a Monday.
    assert_eq!(
        next(&weekly, "2026-05-04T00:00:00Z"),
        Some(ms("2026-05-08T18:30:00Z"))
    );
    assert_eq!(
        next(&weekly, "2026-05-08T18:30:00Z"),
        Some(ms("2026-05-15T18:30:00Z"))
    );
}

#[test]
fn monthly_fires_on_its_day_and_crosses_the_year() {
    let mut monthly = schedule(Cadence::Monthly, "07:15", Some("UTC"));
    monthly.day_of_month = Some(28);
    assert_eq!(
        next(&monthly, "2026-12-28T07:15:00Z"),
        Some(ms("2027-01-28T07:15:00Z"))
    );
    assert_eq!(
        next(&monthly, "2027-02-01T00:00:00Z"),
        Some(ms("2027-02-28T07:15:00Z"))
    );
}

#[test]
fn once_fires_only_when_it_is_still_ahead() {
    let mut once = schedule(Cadence::Once, "12:00", Some("UTC"));
    once.date = Some("2026-06-01".to_owned());
    assert_eq!(
        next(&once, "2026-05-01T00:00:00Z"),
        Some(ms("2026-06-01T12:00:00Z"))
    );
    assert_eq!(next(&once, "2026-06-01T12:00:00Z"), None);
    assert_eq!(next(&once, "2027-01-01T00:00:00Z"), None);
}

#[test]
fn latest_occurrence_at_or_before_an_instant() {
    let daily = schedule(Cadence::Daily, "09:00", Some("UTC"))
        .compile()
        .expect("valid");
    assert_eq!(
        daily.latest_at_or_before(ms("2026-05-04T09:00:00Z")),
        Some(ms("2026-05-04T09:00:00Z"))
    );
    assert_eq!(
        daily.latest_at_or_before(ms("2026-05-04T08:59:59Z")),
        Some(ms("2026-05-03T09:00:00Z"))
    );
}

#[test]
fn searches_end_at_the_edges_of_time() {
    let mut monthly = schedule(Cadence::Monthly, "00:00", Some("UTC"));
    monthly.day_of_month = Some(1);
    let monthly = monthly.compile().expect("valid");
    assert_eq!(monthly.next_after(i64::MAX), None);
    assert_eq!(monthly.next_after(i64::MIN), None);
    assert_eq!(monthly.latest_at_or_before(i64::MIN), None);
    assert_eq!(monthly.next_after(ms("9999-12-01T00:00:00Z")), None);
}

#[test]
fn no_zone_uses_the_host_zone() {
    let daily = schedule(Cadence::Daily, "09:00", None)
        .compile()
        .expect("valid");
    assert!(matches!(
        daily.zone_source(),
        ZoneSource::Host | ZoneSource::HostUnknownUsedUtc
    ));
    assert!(daily.next_after(ms("2026-05-04T00:00:00Z")).is_some());
}

#[test]
fn named_zone_reports_its_source() {
    let daily = schedule(Cadence::Daily, "09:00", Some("America/New_York"))
        .compile()
        .expect("valid");
    assert_eq!(daily.zone_source(), ZoneSource::Named);
}

#[test]
fn validation_names_each_mistake() {
    let compile = |s: Schedule| s.compile().err();

    assert_eq!(
        compile(schedule(Cadence::Weekly, "09:00", None)),
        Some(ScheduleError::WeeklyWithoutWeekday)
    );
    assert_eq!(
        compile(schedule(Cadence::Monthly, "09:00", None)),
        Some(ScheduleError::MonthlyWithoutDay)
    );
    for day in [0, 29, 31] {
        let mut monthly = schedule(Cadence::Monthly, "09:00", None);
        monthly.day_of_month = Some(day);
        assert_eq!(
            compile(monthly),
            Some(ScheduleError::MonthlyDayOutOfRange(day))
        );
    }
    assert_eq!(
        compile(schedule(Cadence::Once, "09:00", None)),
        Some(ScheduleError::OnceWithoutDate)
    );
    for date in ["2026-02-30", "2026/01/01", "26-01-01", ""] {
        let mut once = schedule(Cadence::Once, "09:00", None);
        once.date = Some(date.to_owned());
        assert_eq!(compile(once), Some(ScheduleError::BadDate(date.to_owned())));
    }
    for time in ["24:00", "9:05", "09:60", "ab:cd", "09:00:00", ""] {
        assert_eq!(
            compile(schedule(Cadence::Daily, time, None)),
            Some(ScheduleError::BadTime(time.to_owned()))
        );
    }
    assert_eq!(
        compile(schedule(Cadence::Daily, "09:00", Some("Not/A_Zone"))),
        Some(ScheduleError::UnknownZone("Not/A_Zone".to_owned()))
    );
    let long = "A".repeat(65);
    assert_eq!(
        compile(schedule(Cadence::Daily, "09:00", Some(&long))),
        Some(ScheduleError::ZoneTooLong(65))
    );
}

#[test]
fn schedule_round_trips_through_json() {
    let mut weekly = schedule(Cadence::Weekly, "18:30", Some("Europe/Madrid"));
    weekly.weekday = Some(Weekday::Sunday);
    let text = serde_json::to_string(&weekly).expect("serializes");
    assert!(text.contains("\"cadence\":\"weekly\""), "{text}");
    assert!(text.contains("\"weekday\":\"sunday\""), "{text}");
    let back: Schedule = serde_json::from_str(&text).expect("deserializes");
    assert_eq!(back, weekly);
}
