//! The window `massorca calendar` asks for: a UTC date and a number of days, as the protocol's milliseconds.

use massorca_protocol::codes;
use massorca_protocol::methods::calendar::{
    CalendarKind, CalendarOccurrencesParams, MAX_WINDOW_DAYS,
};

use crate::error::{CliError, exit};

const DAY_MS: i64 = 86_400_000;

/// The params for `days` days from `from` (a UTC date, `YYYY-MM-DD`), or from today in UTC when `from` is unset.
pub fn params(
    from: Option<&str>,
    days: i64,
    kind: Option<CalendarKind>,
    include_done: bool,
    now_ms: i64,
) -> Result<CalendarOccurrencesParams, CliError> {
    if !(1..=MAX_WINDOW_DAYS).contains(&days) {
        return Err(CliError::new(
            codes::REQUEST_INVALID_PARAMS,
            format!("--days must be from 1 to {MAX_WINDOW_DAYS}"),
            exit::ERROR,
        ));
    }
    let from_ms = match from {
        Some(text) => date_ms(text).ok_or_else(|| {
            CliError::new(
                codes::REQUEST_INVALID_PARAMS,
                "--from must be a UTC date such as 2026-11-25",
                exit::ERROR,
            )
        })?,
        None => now_ms.div_euclid(DAY_MS) * DAY_MS,
    };
    Ok(CalendarOccurrencesParams {
        from_ms,
        to_ms: from_ms + days * DAY_MS,
        kind,
        include_done,
    })
}

/// The UTC midnight, in milliseconds, of a real `YYYY-MM-DD` date.
pub fn date_ms(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year = digits(&bytes[0..4])?;
    let month = digits(&bytes[5..7])?;
    let day = digits(&bytes[8..10])?;
    if !(1..=12).contains(&month) || !(1..=days_in_month(year, month)).contains(&day) {
        return None;
    }
    Some(days_from_civil(year, month, day) * DAY_MS)
}

fn digits(bytes: &[u8]) -> Option<i64> {
    bytes.iter().try_fold(0_i64, |acc, byte| {
        byte.is_ascii_digit()
            .then(|| acc * 10 + i64::from(byte - b'0'))
    })
}

fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 31,
    }
}

/// The days from 1970-01-01 to a civil date (Howard Hinnant's `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_from_march = (month + 9) % 12;
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::calendar::CalendarKind;

    use super::{DAY_MS, date_ms, params};

    /// 2026-11-25 00:00 UTC.
    const NOV_25: i64 = 1_795_564_800_000;

    #[test]
    fn a_date_is_the_utc_midnight_of_that_day() {
        assert_eq!(date_ms("2026-11-25"), Some(NOV_25));
        assert_eq!(date_ms("1970-01-01"), Some(0));
    }

    #[test]
    fn only_real_dates_are_accepted() {
        assert_eq!(date_ms("2026-02-29"), None, "2026 is not a leap year");
        assert!(date_ms("2028-02-29").is_some());
        assert_eq!(date_ms("2026-13-01"), None);
        assert_eq!(date_ms("2026-4-1"), None);
        assert_eq!(date_ms("26-11-25"), None);
    }

    #[test]
    fn the_window_starts_at_the_date_and_spans_the_days_asked_for() {
        let asked = params(Some("2026-11-25"), 3, Some(CalendarKind::Routine), true, 0)
            .expect("a valid window");
        assert_eq!(asked.from_ms, NOV_25);
        assert_eq!(asked.to_ms, NOV_25 + 3 * DAY_MS);
        assert_eq!(asked.kind, Some(CalendarKind::Routine));
        assert!(asked.include_done);
    }

    #[test]
    fn without_a_date_the_window_starts_at_today_in_utc() {
        let asked = params(None, 7, None, false, NOV_25 + 5 * 3_600_000).expect("a valid window");
        assert_eq!(asked.from_ms, NOV_25);
    }

    #[test]
    fn the_days_must_be_from_one_to_sixty_two() {
        assert!(params(Some("2026-11-25"), 0, None, false, 0).is_err());
        assert!(params(Some("2026-11-25"), 63, None, false, 0).is_err());
        assert!(params(Some("2026-11-25"), 62, None, false, 0).is_ok());
    }
}
