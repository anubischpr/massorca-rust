//! The week the Calendar shows: its days, its title, and the window of instants it fetches (decision 0124).

use jiff::civil::{Date, Weekday};
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};

/// Today, in the host's zone.
pub fn today() -> Date {
    let zone = TimeZone::try_system().unwrap_or(TimeZone::UTC);
    zone.to_datetime(Timestamp::now()).date()
}

/// The Monday of the week that holds `day`.
pub fn week_start(day: Date) -> Date {
    let back = i64::from(day.weekday().to_monday_zero_offset());
    day.checked_sub(Span::new().days(back)).unwrap_or(day)
}

/// The seven days of the week that starts on `start`, Monday first.
pub fn week_days(start: Date) -> [Date; 7] {
    let mut day = start;
    std::array::from_fn(|_| {
        let current = day;
        day = current.tomorrow().unwrap_or(current);
        current
    })
}

/// The column of the week a local date falls in, or `None` when it is outside the week.
pub fn column_of(local_date: &str, start: Date) -> Option<usize> {
    let day: Date = local_date.parse().ok()?;
    week_days(start)
        .iter()
        .position(|candidate| *candidate == day)
}

/// The instants the week's runs are fetched between, in the host's zone: from the day before its Monday to the day after
/// its Sunday, so a run placed on a neighbouring local date is fetched too. Nine days, within the protocol's 62.
pub fn fetch_window(start: Date) -> Option<(i64, i64)> {
    let zone = TimeZone::try_system().unwrap_or(TimeZone::UTC);
    let from = start
        .yesterday()
        .ok()?
        .to_zoned_datetime(zone.clone())
        .ok()?;
    let to = start
        .checked_add(Span::new().days(8))
        .ok()?
        .to_zoned_datetime(zone)
        .ok()?;
    Some((
        from.timestamp().as_millisecond(),
        to.timestamp().as_millisecond(),
    ))
}

/// The week's title, as the artboard words it: "9 to 15 Nov 2026".
pub fn range_title(start: Date) -> String {
    let end = start.checked_add(Span::new().days(6)).unwrap_or(start);
    if start.month() == end.month() {
        format!(
            "{} to {} {} {}",
            start.day(),
            end.day(),
            month_name(end.month()),
            end.year()
        )
    } else if start.year() == end.year() {
        format!(
            "{} {} to {} {} {}",
            start.day(),
            month_name(start.month()),
            end.day(),
            month_name(end.month()),
            end.year()
        )
    } else {
        format!(
            "{} {} {} to {} {} {}",
            start.day(),
            month_name(start.month()),
            start.year(),
            end.day(),
            month_name(end.month()),
            end.year()
        )
    }
}

/// A column heading: the weekday and the day of the month, as "Mon 23".
pub fn day_heading(day: Date) -> String {
    format!("{} {}", weekday_name(day.weekday()), day.day())
}

fn month_name(month: i8) -> &'static str {
    const NAMES: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    usize::try_from(month - 1)
        .ok()
        .and_then(|index| NAMES.get(index).copied())
        .unwrap_or("")
}

fn weekday_name(day: Weekday) -> &'static str {
    match day {
        Weekday::Monday => "Mon",
        Weekday::Tuesday => "Tue",
        Weekday::Wednesday => "Wed",
        Weekday::Thursday => "Thu",
        Weekday::Friday => "Fri",
        Weekday::Saturday => "Sat",
        Weekday::Sunday => "Sun",
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::{column_of, day_heading, fetch_window, range_title, week_days, week_start};

    #[test]
    fn the_week_of_a_wednesday_starts_on_the_monday_before_and_a_monday_starts_its_own() {
        assert_eq!(week_start(date(2026, 11, 25)), date(2026, 11, 23));
        assert_eq!(week_start(date(2026, 11, 23)), date(2026, 11, 23));
        assert_eq!(week_start(date(2026, 11, 29)), date(2026, 11, 23));
    }

    #[test]
    fn a_week_has_seven_days_from_its_monday_and_a_local_date_finds_its_column() {
        let start = date(2026, 11, 23);
        let days = week_days(start);
        assert_eq!(days[0], start);
        assert_eq!(days[6], date(2026, 11, 29));
        assert_eq!(column_of("2026-11-25", start), Some(2));
        assert_eq!(column_of("2026-11-30", start), None);
        assert_eq!(column_of("not a date", start), None);
    }

    #[test]
    fn the_title_names_the_days_the_week_covers() {
        assert_eq!(range_title(date(2026, 11, 9)), "9 to 15 Nov 2026");
        assert_eq!(range_title(date(2026, 11, 30)), "30 Nov to 6 Dec 2026");
        assert_eq!(range_title(date(2026, 12, 28)), "28 Dec 2026 to 3 Jan 2027");
    }

    #[test]
    fn a_column_heading_names_the_weekday_and_the_day() {
        assert_eq!(day_heading(date(2026, 11, 23)), "Mon 23");
    }

    #[test]
    fn the_fetch_window_covers_nine_days_and_no_more() {
        let (from, to) = fetch_window(date(2026, 11, 23)).expect("a window in the host's zone");
        assert_eq!((to - from) / 86_400_000, 9);
    }
}
