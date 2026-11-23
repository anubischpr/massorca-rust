//! The stored schedule shape and its validation.

use jiff::civil::{Date, Time};
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};

use crate::occurrence::CompiledSchedule;
use crate::zone::{ZoneSource, host_zone};

/// Longest zone name accepted; the longest IANA name is well under this.
pub const MAX_ZONE_NAME_LEN: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cadence {
    Daily,
    /// Needs [`Schedule::weekday`].
    Weekly,
    /// Needs [`Schedule::day_of_month`] in 1..=28, so every month has it.
    Monthly,
    /// Needs [`Schedule::date`].
    Once,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    pub(crate) fn to_jiff(self) -> jiff::civil::Weekday {
        match self {
            Self::Monday => jiff::civil::Weekday::Monday,
            Self::Tuesday => jiff::civil::Weekday::Tuesday,
            Self::Wednesday => jiff::civil::Weekday::Wednesday,
            Self::Thursday => jiff::civil::Weekday::Thursday,
            Self::Friday => jiff::civil::Weekday::Friday,
            Self::Saturday => jiff::civil::Weekday::Saturday,
            Self::Sunday => jiff::civil::Weekday::Sunday,
        }
    }
}

/// A schedule as a store keeps it. Fields a cadence does not use are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    pub cadence: Cadence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekday: Option<Weekday>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day_of_month: Option<u8>,
    /// `YYYY-MM-DD`, for [`Cadence::Once`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// Wall-clock `HH:MM`, 24-hour.
    pub time: String,
    /// IANA zone name; `None` means the host's zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScheduleError {
    #[error("a weekly schedule needs a weekday")]
    WeeklyWithoutWeekday,
    #[error("a monthly schedule needs a day of the month")]
    MonthlyWithoutDay,
    #[error("day of the month {0} is outside 1 to 28")]
    MonthlyDayOutOfRange(u8),
    #[error("a one-time schedule needs a date")]
    OnceWithoutDate,
    #[error("date {0:?} is not a real YYYY-MM-DD date")]
    BadDate(String),
    #[error("time {0:?} is not a 24-hour HH:MM time")]
    BadTime(String),
    #[error("time zone {0:?} is unknown")]
    UnknownZone(String),
    #[error("time zone name is {0} characters, more than 64")]
    ZoneTooLong(usize),
}

/// The cadence with its parameters checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rule {
    Daily,
    Weekly(jiff::civil::Weekday),
    Monthly(i8),
    Once(Date),
}

impl Schedule {
    /// Checks every field and resolves the zone (the host's when `zone` is `None`).
    pub fn compile(&self) -> Result<CompiledSchedule, ScheduleError> {
        let rule = match self.cadence {
            Cadence::Daily => Rule::Daily,
            Cadence::Weekly => Rule::Weekly(
                self.weekday
                    .ok_or(ScheduleError::WeeklyWithoutWeekday)?
                    .to_jiff(),
            ),
            Cadence::Monthly => {
                let day = self.day_of_month.ok_or(ScheduleError::MonthlyWithoutDay)?;
                match i8::try_from(day) {
                    Ok(d @ 1..=28) => Rule::Monthly(d),
                    _ => return Err(ScheduleError::MonthlyDayOutOfRange(day)),
                }
            }
            Cadence::Once => {
                let date = self.date.as_deref().ok_or(ScheduleError::OnceWithoutDate)?;
                Rule::Once(parse_date(date).ok_or_else(|| ScheduleError::BadDate(date.to_owned()))?)
            }
        };
        let time =
            parse_time(&self.time).ok_or_else(|| ScheduleError::BadTime(self.time.clone()))?;
        let (zone, source) = match self.zone.as_deref() {
            None => host_zone(),
            Some(name) if name.chars().count() > MAX_ZONE_NAME_LEN => {
                return Err(ScheduleError::ZoneTooLong(name.chars().count()));
            }
            Some(name) => (
                TimeZone::get(name).map_err(|_| ScheduleError::UnknownZone(name.to_owned()))?,
                ZoneSource::Named,
            ),
        };
        Ok(CompiledSchedule::new(rule, time, zone, source))
    }
}

fn two_digits(bytes: &[u8]) -> Option<i8> {
    match bytes {
        [a @ b'0'..=b'9', b @ b'0'..=b'9'] => i8::try_from((a - b'0') * 10 + (b - b'0')).ok(),
        _ => None,
    }
}

fn parse_time(text: &str) -> Option<Time> {
    match text.as_bytes() {
        [h1, h2, b':', m1, m2] => {
            let hour = two_digits(&[*h1, *h2])?;
            let minute = two_digits(&[*m1, *m2])?;
            Time::new(hour, minute, 0, 0).ok()
        }
        _ => None,
    }
}

fn parse_date(text: &str) -> Option<Date> {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year = bytes[..4].iter().try_fold(0i16, |acc, b| {
        b.is_ascii_digit().then(|| acc * 10 + i16::from(b - b'0'))
    })?;
    let month = two_digits(&bytes[5..7])?;
    let day = two_digits(&bytes[8..10])?;
    Date::new(year, month, day).ok()
}
