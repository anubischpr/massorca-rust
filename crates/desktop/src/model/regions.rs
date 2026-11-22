//! Region widths and the collapse order (design-system 5.1, handoff 2.5).

use massorca_ui::tokens::{
    LAYOUT_PROJECT_DEFAULT, LAYOUT_PROJECT_MAX, LAYOUT_PROJECT_MIN, LAYOUT_SIDEBAR_DEFAULT,
    LAYOUT_SIDEBAR_MAX, LAYOUT_SIDEBAR_MIN, LAYOUT_SUMMARY_DEFAULT, LAYOUT_SUMMARY_MAX,
    LAYOUT_SUMMARY_MIN, LAYOUT_WORKPANEL_DEFAULT, LAYOUT_WORKPANEL_MIN, SIZE_RAIL_WIDTH,
};

/// The collapsible regions of the desktop window, in collapse order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Region {
    SessionSummary,
    ProjectSidebar,
    Sidebar,
    WorkPanel,
}

impl Region {
    /// Collapse order when the window narrows: first gives way first.
    pub const COLLAPSE_ORDER: [Region; 4] = [
        Region::SessionSummary,
        Region::ProjectSidebar,
        Region::Sidebar,
        Region::WorkPanel,
    ];

    /// Window width below which this region stops being inline (X1; artboard ws-narrow steps).
    pub fn threshold(self) -> f32 {
        match self {
            Region::SessionSummary => 1408.0,
            Region::ProjectSidebar => 1168.0,
            Region::Sidebar => 928.0,
            Region::WorkPanel => 728.0,
        }
    }

    /// The glossary name shown in copy and landmarks (A5, A91).
    pub fn name(self) -> &'static str {
        match self {
            Region::SessionSummary => "Session summary",
            Region::ProjectSidebar => "Project sidebar",
            Region::Sidebar => "Sidebar",
            Region::WorkPanel => "Work panel",
        }
    }

    pub fn default_width(self) -> f32 {
        match self {
            Region::SessionSummary => LAYOUT_SUMMARY_DEFAULT,
            Region::ProjectSidebar => LAYOUT_PROJECT_DEFAULT,
            Region::Sidebar => LAYOUT_SIDEBAR_DEFAULT,
            Region::WorkPanel => LAYOUT_WORKPANEL_DEFAULT,
        }
    }

    pub fn min_width(self) -> f32 {
        match self {
            Region::SessionSummary => LAYOUT_SUMMARY_MIN,
            Region::ProjectSidebar => LAYOUT_PROJECT_MIN,
            Region::Sidebar => LAYOUT_SIDEBAR_MIN,
            Region::WorkPanel => LAYOUT_WORKPANEL_MIN,
        }
    }

    /// `None` means "the rest of the width" (Work panel).
    pub fn max_width(self) -> Option<f32> {
        match self {
            Region::SessionSummary => Some(LAYOUT_SUMMARY_MAX),
            Region::ProjectSidebar => Some(LAYOUT_PROJECT_MAX),
            Region::Sidebar => Some(LAYOUT_SIDEBAR_MAX),
            Region::WorkPanel => None,
        }
    }
}

/// How a region the user wants open is shown at the current window width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// Closed by the user.
    Closed,
    /// Inline in the layout.
    Inline,
    /// Collapsed by width; opens as an overlay from its title bar toggle.
    Collapsed,
}

/// Regions the user has open, kept while the window is narrow so they come
/// back inline when it grows again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionState {
    open: Vec<Region>,
    /// The one region shown as an overlay, if any.
    overlay: Option<Region>,
}

impl Default for RegionState {
    fn default() -> Self {
        Self {
            open: vec![Region::Sidebar],
            overlay: None,
        }
    }
}

impl RegionState {
    pub fn is_open(&self, region: Region) -> bool {
        self.open.contains(&region)
    }

    /// Opens or closes a region. Narrow: an open region toggles its overlay, and a
    /// closed one opens as the overlay (handoff 2.5).
    pub fn toggle(&mut self, region: Region, window_width: f32) {
        let narrow = window_width < region.threshold();
        if self.placement(region, window_width) == Placement::Collapsed {
            self.overlay = if self.overlay == Some(region) {
                None
            } else {
                Some(region)
            };
            return;
        }
        if let Some(i) = self.open.iter().position(|r| *r == region) {
            self.open.remove(i);
        } else {
            self.open.push(region);
            if narrow {
                self.overlay = Some(region);
            }
        }
    }

    pub fn placement(&self, region: Region, window_width: f32) -> Placement {
        if !self.is_open(region) {
            Placement::Closed
        } else if window_width < region.threshold() {
            Placement::Collapsed
        } else {
            Placement::Inline
        }
    }

    /// The overlay region, if one is open; Escape or a click outside clears it.
    pub fn overlay(&self) -> Option<Region> {
        self.overlay
    }

    pub fn close_overlay(&mut self) {
        self.overlay = None;
    }
}

/// Width of the fixed Activity rail.
pub const RAIL_WIDTH: f32 = SIZE_RAIL_WIDTH;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_closed_region_opened_while_narrow_appears_as_the_overlay() {
        let mut s = RegionState::default();
        s.toggle(Region::SessionSummary, 900.0);
        assert!(s.is_open(Region::SessionSummary));
        assert_eq!(s.overlay(), Some(Region::SessionSummary));
        assert_eq!(
            s.placement(Region::SessionSummary, 900.0),
            Placement::Collapsed
        );
    }

    #[test]
    fn region_names_follow_the_glossary() {
        assert_eq!(Region::SessionSummary.name(), "Session summary");
        assert_eq!(Region::WorkPanel.name(), "Work panel");
        assert_eq!(Region::ProjectSidebar.name(), "Project sidebar");
        assert_eq!(Region::Sidebar.name(), "Sidebar");
    }

    /// The region is inline at `width` and gives way one pixel narrower.
    fn gives_way_below(region: Region, width: f32) {
        let mut s = RegionState::default();
        if !s.is_open(region) {
            s.toggle(region, 2000.0);
        }
        assert_eq!(s.placement(region, width), Placement::Inline);
        assert_eq!(s.placement(region, width - 1.0), Placement::Collapsed);
    }

    #[test]
    fn session_summary_gives_way_below_1408() {
        gives_way_below(Region::SessionSummary, 1408.0);
    }

    #[test]
    fn project_sidebar_gives_way_below_1168() {
        gives_way_below(Region::ProjectSidebar, 1168.0);
    }

    #[test]
    fn sidebar_gives_way_below_928() {
        gives_way_below(Region::Sidebar, 928.0);
    }

    #[test]
    fn work_panel_gives_way_below_728() {
        gives_way_below(Region::WorkPanel, 728.0);
    }

    #[test]
    fn the_collapse_order_follows_the_artboard_steps() {
        assert_eq!(
            Region::COLLAPSE_ORDER.map(Region::threshold),
            [1408.0, 1168.0, 928.0, 728.0]
        );
    }

    #[test]
    fn regions_collapse_in_order_and_come_back_when_wide() {
        let mut s = RegionState::default();
        for r in Region::COLLAPSE_ORDER {
            if !s.is_open(r) {
                s.toggle(r, 2000.0);
            }
        }
        let at = |w| Region::COLLAPSE_ORDER.map(|r| s.placement(r, w));
        assert!(at(1600.0).iter().all(|p| *p == Placement::Inline));
        assert_eq!(at(1400.0)[0], Placement::Collapsed);
        assert_eq!(at(1400.0)[1], Placement::Inline);
        // At the 800 px window minimum only the Work panel can stay inline (step 4 needs zoom).
        assert_eq!(
            at(800.0),
            [
                Placement::Collapsed,
                Placement::Collapsed,
                Placement::Collapsed,
                Placement::Inline
            ]
        );
        assert!(at(700.0).iter().all(|p| *p == Placement::Collapsed));
        assert!(at(1600.0).iter().all(|p| *p == Placement::Inline));
    }

    #[test]
    fn a_collapsed_region_toggles_as_the_single_overlay() {
        let mut s = RegionState::default();
        s.toggle(Region::SessionSummary, 2000.0);
        s.toggle(Region::SessionSummary, 1000.0);
        assert_eq!(s.overlay(), Some(Region::SessionSummary));
        s.toggle(Region::Sidebar, 900.0);
        assert_eq!(
            s.overlay(),
            Some(Region::Sidebar),
            "only one overlay at a time"
        );
        assert!(
            s.is_open(Region::SessionSummary),
            "the user's choice is kept"
        );
        s.close_overlay();
        assert_eq!(s.overlay(), None);
    }
}
