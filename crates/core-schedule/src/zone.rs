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

/// The zone a schedule without a zone name uses on this host now: its IANA name (`UTC` when the host's zone cannot be
/// read, `None` when the host's zone has no IANA name), and where it came from.
pub fn host_zone_used() -> (Option<String>, ZoneSource) {
    let (zone, source) = host_zone();
    (zone.iana_name().map(str::to_owned), source)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_host_zone_used_is_the_zone_a_schedule_without_a_zone_resolves_to() {
        let (name, source) = super::host_zone_used();
        let (zone, expected) = super::host_zone();
        assert_eq!(source, expected);
        assert_eq!(name.as_deref(), zone.iana_name());
    }
}
