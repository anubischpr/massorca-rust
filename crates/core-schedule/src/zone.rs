//! Which zone a schedule without a zone name uses.

use jiff::tz::TimeZone;

/// Where a compiled schedule's zone came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneSource {
    /// The schedule named an IANA zone.
    Named,
    /// The host's zone (`TZ`, then the operating system setting).
    Host,
    /// The host's zone could not be read, so UTC is used. Callers may want to say so.
    HostUnknownUsedUtc,
}

pub(crate) fn host_zone() -> (TimeZone, ZoneSource) {
    match TimeZone::try_system() {
        Ok(zone) => (zone, ZoneSource::Host),
        Err(_) => (TimeZone::UTC, ZoneSource::HostUnknownUsedUtc),
    }
}
