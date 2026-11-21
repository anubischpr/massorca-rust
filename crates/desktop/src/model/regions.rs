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

    /// Window width below which this region stops being inline (X1).
    pub fn threshold(self) -> f32 {
        match self {
            Region::SessionSummary => 1528.0,
            Region::ProjectSidebar => 1288.0,
            Region::Sidebar => 1048.0,
            Region::WorkPanel => 848.0,
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

    pub fn toggle(&mut self, region: Region, window_width: f32) {
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
    fn thresholds_are_sums_of_the_open_minimums() {
        // Rail 48 + Sidebar 200 + Conversation 480 + Work panel 320 + two 240 columns.
        assert_eq!(48.0 + 200.0 + 480.0 + 320.0 + 240.0 + 240.0, 1528.0);
        assert_eq!(Region::SessionSummary.threshold(), 1528.0);
        assert_eq!(48.0 + 480.0 + 320.0, Region::WorkPanel.threshold());
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
        assert!(at(800.0).iter().all(|p| *p == Placement::Collapsed));
        assert!(at(1600.0).iter().all(|p| *p == Placement::Inline));
    }

    #[test]
    fn a_collapsed_region_toggles_as_the_single_overlay() {
        let mut s = RegionState::default();
        s.toggle(Region::SessionSummary, 2000.0);
        s.toggle(Region::SessionSummary, 1000.0);
        assert_eq!(s.overlay(), Some(Region::SessionSummary));
        s.toggle(Region::Sidebar, 1000.0);
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
