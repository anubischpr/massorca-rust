//! Five-field cron expressions (decision 0123): minute, hour, day of month, month and day of week.
//!
//! Each field takes numbers, lists (`,`), ranges (`-`) and steps (`/`); months and weekdays also take their
//! three-letter names. Day of week 7 is Sunday, as 0 is. A day field that starts with `*` is unrestricted; when both
//! day fields are restricted, a day matches when either does, as standard cron reads them.

use jiff::civil::{Date, Time};
use serde::{Deserialize, Serialize};

use crate::occurrence::CompiledSchedule;
use crate::schedule::{Rule, ScheduleError, resolve_zone};

/// Longest cron expression accepted, in bytes.
pub const MAX_CRON_EXPRESSION_LEN: usize = 256;

/// A cron expression and the zone it runs in. No zone means the host's zone, as for [`crate::Schedule`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronSchedule {
    /// Five fields: minute, hour, day of month, month and day of week.
    pub expression: String,
    /// IANA zone name; `None` means the host's zone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
}

impl CronSchedule {
    /// Checks the expression, refusing one that can never occur, and resolves the zone.
    pub fn compile(&self) -> Result<CompiledSchedule, ScheduleError> {
        let fields = parse_expression(&self.expression)?;
        let (zone, source) = resolve_zone(self.zone.as_deref())?;
        Ok(CompiledSchedule::new(
            Rule::Cron(fields.days),
            fields.times(),
            zone,
            source,
        ))
    }
}

/// The days a cron expression names: each field as a bit set, where bit `v` is the value `v`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CronDays {
    months: u64,
    days_of_month: u64,
    /// Bit 0 is Sunday.
    days_of_week: u64,
    days_of_month_restricted: bool,
    days_of_week_restricted: bool,
}

impl CronDays {
    /// Whether `date` is one of the days the expression names.
    pub(crate) fn matches(self, date: Date) -> bool {
        let month = contains(self.months, date.month());
        let day_of_month = contains(self.days_of_month, date.day());
        let day_of_week = contains(self.days_of_week, date.weekday().to_sunday_zero_offset());
        let day = if self.days_of_month_restricted && self.days_of_week_restricted {
            day_of_month || day_of_week
        } else {
            day_of_month && day_of_week
        };
        month && day
    }
}

/// Everything a valid expression names: its minutes, its hours and its days.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fields {
    minutes: u64,
    hours: u64,
    days: CronDays,
}

impl Fields {
    /// Every wall-clock time of a matching day, earliest first.
    fn times(self) -> Vec<Time> {
        let mut times = Vec::new();
        for hour in 0..24_i8 {
            if !contains(self.hours, hour) {
                continue;
            }
            for minute in 0..60_i8 {
                if contains(self.minutes, minute) {
                    times.extend(Time::new(hour, minute, 0, 0).ok());
                }
            }
        }
        times
    }
}

/// One field's syntax: its name in errors, its values, and the names it accepts.
struct Spec {
    name: &'static str,
    min: u8,
    max: u8,
    names: &'static [&'static str],
    /// The value of the first name: 1 for months, 0 for weekdays.
    name_base: u8,
}

const MINUTE: Spec = Spec {
    name: "minute",
    min: 0,
    max: 59,
    names: &[],
    name_base: 0,
};
const HOUR: Spec = Spec {
    name: "hour",
    min: 0,
    max: 23,
    names: &[],
    name_base: 0,
};
const DAY_OF_MONTH: Spec = Spec {
    name: "day of month",
    min: 1,
    max: 31,
    names: &[],
    name_base: 0,
};
const MONTH: Spec = Spec {
    name: "month",
    min: 1,
    max: 12,
    names: &[
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ],
    name_base: 1,
};
const DAY_OF_WEEK: Spec = Spec {
    name: "day of week",
    min: 0,
    max: 7,
    names: &["sun", "mon", "tue", "wed", "thu", "fri", "sat"],
    name_base: 0,
};

/// The bit for Sunday written as 7, which folds into 0.
const SUNDAY_AS_SEVEN: u64 = 1 << 7;

/// Longest month, February counted as 29 days so a leap day is possible.
fn longest_day(month: i8) -> i8 {
    match month {
        2 => 29,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn parse_expression(expression: &str) -> Result<Fields, ScheduleError> {
    if expression.len() > MAX_CRON_EXPRESSION_LEN {
        return Err(ScheduleError::CronTooLong(expression.len()));
    }
    let parts: Vec<&str> = expression.split_whitespace().collect();
    let &[minute, hour, day_of_month, month, day_of_week] = parts.as_slice() else {
        return Err(ScheduleError::CronFieldCount(parts.len()));
    };
    let days_of_week = field(day_of_week, &DAY_OF_WEEK)?;
    let fields = Fields {
        minutes: field(minute, &MINUTE)?,
        hours: field(hour, &HOUR)?,
        days: CronDays {
            months: field(month, &MONTH)?,
            days_of_month: field(day_of_month, &DAY_OF_MONTH)?,
            days_of_week: fold_sunday(days_of_week),
            days_of_month_restricted: !day_of_month.starts_with('*'),
            days_of_week_restricted: !day_of_week.starts_with('*'),
        },
    };
    if !occurs(fields.days) {
        return Err(ScheduleError::CronNeverOccurs);
    }
    Ok(fields)
}

/// Whether the expression names some day of the calendar. Only a day of the month alone can name none: a day that
/// some months never have, such as 31 April. A weekday always occurs in every month.
fn occurs(days: CronDays) -> bool {
    if days.days_of_week_restricted || !days.days_of_month_restricted {
        return true;
    }
    (1..=12_i8).any(|month| {
        contains(days.months, month)
            && (1..=longest_day(month)).any(|day| contains(days.days_of_month, day))
    })
}

/// The set one field names, checked against its range.
fn field(text: &str, spec: &Spec) -> Result<u64, ScheduleError> {
    let refused = || ScheduleError::CronField {
        field: spec.name,
        text: text.to_owned(),
    };
    let mut set = 0u64;
    for item in text.split(',') {
        let (range, step) = match item.split_once('/') {
            Some((range, step)) => (range, Some(step)),
            None => (item, None),
        };
        let (first, last) = if range == "*" {
            (spec.min, spec.max)
        } else if let Some((from, to)) = range.split_once('-') {
            (
                value(from, spec).ok_or_else(refused)?,
                value(to, spec).ok_or_else(refused)?,
            )
        } else {
            let first = value(range, spec).ok_or_else(refused)?;
            // `A/S` runs from A to the field's last value, as in `A-max/S`.
            (first, if step.is_some() { spec.max } else { first })
        };
        let step = match step {
            None => 1,
            Some(text_step) => {
                let parsed = digits(text_step)
                    .filter(|step| *step >= 1)
                    .ok_or_else(refused)?;
                if parsed > spec.max - spec.min {
                    return Err(ScheduleError::CronStepTooLarge {
                        field: spec.name,
                        step: parsed,
                    });
                }
                parsed
            }
        };
        if first < spec.min || last > spec.max || first > last {
            return Err(refused());
        }
        for number in (first..=last).step_by(usize::from(step)) {
            set |= 1 << number;
        }
    }
    Ok(set)
}

/// One value of a field: a number, or a month or weekday name in any letter case.
fn value(token: &str, spec: &Spec) -> Option<u8> {
    if let Some(number) = digits(token) {
        return Some(number);
    }
    let lowered = token.to_ascii_lowercase();
    let index = spec
        .names
        .iter()
        .position(|name| *name == lowered.as_str())?;
    u8::try_from(index).ok().map(|index| index + spec.name_base)
}

/// A plain unsigned number, with no sign and no separators.
fn digits(text: &str) -> Option<u8> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn fold_sunday(set: u64) -> u64 {
    if set & SUNDAY_AS_SEVEN == 0 {
        set
    } else {
        (set & !SUNDAY_AS_SEVEN) | 1
    }
}

/// Whether `set` holds `value`; a negative value never does.
fn contains(set: u64, value: i8) -> bool {
    let bit = u32::from(value.unsigned_abs());
    value >= 0 && bit < 64 && set & (1 << bit) != 0
}
