//! Occurrence search: the next instant after, and the latest at or before, a given instant.

use jiff::Timestamp;
use jiff::civil::{Date, DateTime, Time};
use jiff::tz::{AmbiguousOffset, TimeZone};

use crate::schedule::Rule;
use crate::zone::ZoneSource;

/// Days a search walks before it gives up. Every valid recurring cadence occurs within 31 days.
pub const MAX_SEARCH_DAYS: u32 = 370;

/// A validated schedule with its zone resolved. Instants are UTC epoch milliseconds.
#[derive(Debug, Clone)]
pub struct CompiledSchedule {
    rule: Rule,
    time: Time,
    zone: TimeZone,
    source: ZoneSource,
}

impl CompiledSchedule {
    pub(crate) fn new(rule: Rule, time: Time, zone: TimeZone, source: ZoneSource) -> Self {
        Self {
            rule,
            time,
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
                .resolve(date)
                .filter(|at| *at > after)
                .map(Timestamp::as_millisecond);
        }
        // A day early: a resolved instant can land on the neighbouring local date.
        let start = self.zone.to_datetime(after).date();
        let mut date = start.yesterday().unwrap_or(start);
        for _ in 0..MAX_SEARCH_DAYS {
            if self.matches(date)
                && let Some(at) = self.resolve(date)
                && at > after
            {
                return Some(at.as_millisecond());
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
                .resolve(date)
                .filter(|at| *at <= limit)
                .map(Timestamp::as_millisecond);
        }
        let start = self.zone.to_datetime(limit).date();
        let mut date = start.tomorrow().unwrap_or(start);
        for _ in 0..MAX_SEARCH_DAYS {
            if self.matches(date)
                && let Some(at) = self.resolve(date)
                && at <= limit
            {
                return Some(at.as_millisecond());
            }
            date = date.yesterday().ok()?;
        }
        None
    }

    fn matches(&self, date: Date) -> bool {
        match self.rule {
            Rule::Daily => true,
            Rule::Weekly(weekday) => date.weekday() == weekday,
            Rule::Monthly(day) => date.day() == day,
            Rule::Once(once) => date == once,
        }
    }

    /// The instant the wall time happens on `date`, with the crate's daylight-saving rules.
    fn resolve(&self, date: Date) -> Option<Timestamp> {
        let wall = DateTime::from_parts(date, self.time);
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
