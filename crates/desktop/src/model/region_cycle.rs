//! The regions F6 moves focus through, in order (handoff 2.2). Hidden regions are not in the
//! list, so the cycle skips them; an overlay region joins while it is open.

use super::regions::Region;

/// One part of the frame that F6 can move focus into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    TitleBar,
    Rail,
    Sidebar,
    Conversation,
    SessionSummary,
    WorkPanel,
    ProjectSidebar,
    StatusBar,
}

impl Zone {
    /// The cycle order: Title bar, Activity rail, Sidebar, Conversation, Session summary, Work
    /// panel, Project sidebar, Status bar.
    pub const ORDER: [Zone; 8] = [
        Zone::TitleBar,
        Zone::Rail,
        Zone::Sidebar,
        Zone::Conversation,
        Zone::SessionSummary,
        Zone::WorkPanel,
        Zone::ProjectSidebar,
        Zone::StatusBar,
    ];

    /// The zone's place in `ORDER`, which indexes per-zone state.
    pub fn index(self) -> usize {
        match self {
            Zone::TitleBar => 0,
            Zone::Rail => 1,
            Zone::Sidebar => 2,
            Zone::Conversation => 3,
            Zone::SessionSummary => 4,
            Zone::WorkPanel => 5,
            Zone::ProjectSidebar => 6,
            Zone::StatusBar => 7,
        }
    }

    /// The zone a collapsible region occupies.
    pub fn of_region(region: Region) -> Zone {
        match region {
            Region::Sidebar => Zone::Sidebar,
            Region::SessionSummary => Zone::SessionSummary,
            Region::WorkPanel => Zone::WorkPanel,
            Region::ProjectSidebar => Zone::ProjectSidebar,
        }
    }
}

/// The zone F6 moves to from `current`, forwards or backwards, wrapping at the ends. With no
/// zone focused, forward starts at the first zone and backward at the last.
pub fn next_zone(zones: &[Zone], current: Option<Zone>, backwards: bool) -> Option<Zone> {
    let len = zones.len();
    if len == 0 {
        return None;
    }
    let next = match current.and_then(|zone| zones.iter().position(|z| *z == zone)) {
        Some(index) if backwards => (index + len - 1) % len,
        Some(index) => (index + 1) % len,
        None if backwards => len - 1,
        None => 0,
    };
    zones.get(next).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_order_matches_the_handoff_and_indexes_each_zone_once() {
        for (position, zone) in Zone::ORDER.into_iter().enumerate() {
            assert_eq!(zone.index(), position);
        }
        assert_eq!(Zone::ORDER[0], Zone::TitleBar);
        assert_eq!(Zone::ORDER[7], Zone::StatusBar);
    }

    #[test]
    fn forward_visits_each_zone_then_wraps_to_the_first() {
        let zones = [Zone::TitleBar, Zone::Rail, Zone::StatusBar];
        assert_eq!(next_zone(&zones, None, false), Some(Zone::TitleBar));
        assert_eq!(
            next_zone(&zones, Some(Zone::TitleBar), false),
            Some(Zone::Rail)
        );
        assert_eq!(
            next_zone(&zones, Some(Zone::StatusBar), false),
            Some(Zone::TitleBar)
        );
    }

    #[test]
    fn backward_starts_from_the_last_zone_and_wraps_to_it() {
        let zones = [Zone::TitleBar, Zone::Rail, Zone::StatusBar];
        assert_eq!(next_zone(&zones, None, true), Some(Zone::StatusBar));
        assert_eq!(
            next_zone(&zones, Some(Zone::TitleBar), true),
            Some(Zone::StatusBar)
        );
    }

    #[test]
    fn a_focused_zone_missing_from_the_list_starts_the_cycle_again() {
        let zones = [Zone::TitleBar, Zone::StatusBar];
        assert_eq!(
            next_zone(&zones, Some(Zone::WorkPanel), false),
            Some(Zone::TitleBar)
        );
        assert_eq!(next_zone(&[], None, false), None);
    }

    #[test]
    fn each_collapsible_region_has_its_own_zone() {
        assert_eq!(Zone::of_region(Region::Sidebar), Zone::Sidebar);
        assert_eq!(Zone::of_region(Region::WorkPanel), Zone::WorkPanel);
        assert_eq!(
            Zone::of_region(Region::ProjectSidebar),
            Zone::ProjectSidebar
        );
        assert_eq!(
            Zone::of_region(Region::SessionSummary),
            Zone::SessionSummary
        );
    }
}
