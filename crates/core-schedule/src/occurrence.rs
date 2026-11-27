//! Occurrence search: the next instant after, and the latest at or before, a given instant.

use jiff::Timestamp;
use jiff::civil::{Date, DateTime, Time};
use jiff::tz::{AmbiguousOffset, TimeZone};

use crate::schedule::Rule;
use crate::zone::ZoneSource;

/// Days a search walks before it gives up. Every valid recurring cadence occurs within 31 days.
pub const MAX_SEARCH_DAYS: u32 = 370;

/// Days a cron search walks: nine years, since the longest gap between two leap days is eight years (2096 to 2104).
pub const MAX_CRON_SEARCH_DAYS: u32 = 3_294;

/// A validated schedule with its zone resolved. Instants are UTC epoch milliseconds.
#[derive(Debug, Clone)]
pub struct CompiledSchedule {
    rule: Rule,
    /// The wall-clock times of each matching day, earliest first; never empty.
    times: Vec<Time>,
    zone: TimeZone,
    source: ZoneSource,
}

impl CompiledSchedule {
    pub(crate) fn new(rule: Rule, times: Vec<Time>, zone: TimeZone, source: ZoneSource) -> Self {
        Self {
            rule,
            times,
            zone,
            source,
        }
    }

    pub fn zone_source(&self) -> ZoneSource {
        self.source
    }

    /// The IANA name of the zone in use, when it has one.
    pub fn zone_name(&self) -> Option<&str> {
        self.zone.iana_name()
    }

    /// The first occurrence strictly after `after_ms`, or `None` when there is none
    /// (a past one-time date, or the end of representable time).
    pub fn next_after(&self, after_ms: i64) -> Option<i64> {
        let after = Timestamp::from_millisecond(after_ms).ok()?;
        if let Rule::Once(date) = self.rule {
            return self
                .once_at(date)
                .filter(|at| *at > after)
                .map(Timestamp::as_millisecond);
        }
        // A day early: a resolved instant can land on the neighbouring local date.
        let start = self.zone.to_datetime(after).date();
        let mut date = start.yesterday().unwrap_or(start);
        for _ in 0..self.search_days() {
            if self.matches(date) {
                // Resolved instants rise with the wall time, so the first one after `after` is found by bisection.
                let not_after = self.times.partition_point(|time| {
                    self.resolve(date, *time).is_some_and(|at| at <= after)
                });
                let next = self.times[not_after..]
                    .iter()
                    .find_map(|time| self.resolve(date, *time).filter(|at| *at > after));
                if let Some(at) = next {
                    return Some(at.as_millisecond());
                }
            }
            date = date.tomorrow().ok()?;
        }
        None
    }

    /// The latest occurrence at or before `at_ms`.
    pub fn latest_at_or_before(&self, at_ms: i64) -> Option<i64> {
        let limit = Timestamp::from_millisecond(at_ms).ok()?;
        if let Rule::Once(date) = self.rule {
            return self
                .once_at(date)
                .filter(|at| *at <= limit)
                .map(Timestamp::as_millisecond);
        }
        let start = self.zone.to_datetime(limit).date();
        let mut date = start.tomorrow().unwrap_or(start);
        for _ in 0..self.search_days() {
            if self.matches(date) {
                let not_after = self.times.partition_point(|time| {
                    self.resolve(date, *time).is_some_and(|at| at <= limit)
                });
                let latest = self.times[..not_after]
                    .iter()
                    .rev()
                    .find_map(|time| self.resolve(date, *time).filter(|at| *at <= limit));
                if let Some(at) = latest {
                    return Some(at.as_millisecond());
                }
            }
            date = date.yesterday().ok()?;
        }
        None
    }

    fn search_days(&self) -> u32 {
        match self.rule {
            Rule::Cron(_) => MAX_CRON_SEARCH_DAYS,
            _ => MAX_SEARCH_DAYS,
        }
    }

    fn matches(&self, date: Date) -> bool {
        match self.rule {
            Rule::Daily => true,
            Rule::Weekdays => date.weekday().to_monday_zero_offset() < 5,
            Rule::Weekly(weekday) => date.weekday() == weekday,
            Rule::Monthly(day) => date.day() == day,
            Rule::Once(once) => date == once,
            Rule::Cron(days) => days.matches(date),
        }
    }

    fn once_at(&self, date: Date) -> Option<Timestamp> {
        let time = *self.times.first()?;
        self.resolve(date, time)
    }

    /// The instant the wall time happens on `date`, with the crate's daylight-saving rules.
    fn resolve(&self, date: Date, time: Time) -> Option<Timestamp> {
        let wall = DateTime::from_parts(date, time);
        match self.zone.to_ambiguous_timestamp(wall).offset() {
            AmbiguousOffset::Unambiguous { offset } => offset.to_timestamp(wall).ok(),
            // Skipped wall time: the transition instant, where the clock lands after the jump.
            AmbiguousOffset::Gap { after, .. } => {
                let earliest = after.to_timestamp(wall).ok()?;
                self.zone
                    .following(earliest)
                    .next()
                    .map(|transition| transition.timestamp())
            }
            // Repeated wall time: the earlier of its two instants.
            AmbiguousOffset::Fold { before, after } => {
                let first = before.to_timestamp(wall).ok()?;
                let second = after.to_timestamp(wall).ok()?;
                Some(first.min(second))
            }
        }
    }
}
