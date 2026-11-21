//! The shell's state without GPUI: the page the user is on, the regions they opened
//! and the snapshot the views draw. Actions change it through plain methods.

use super::canvas::Canvas;
use super::navigation::{Navigation, Page};
use super::regions::{Placement, Region, RegionState};
use super::shell::ShellSnapshot;

/// The regions that sit inline beside the Conversation, in the order A5 gives them.
const INLINE_ORDER: [Region; 3] = [
    Region::SessionSummary,
    Region::WorkPanel,
    Region::ProjectSidebar,
];

/// What the shell shows and what the user has changed by acting on it.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellModel {
    snapshot: ShellSnapshot,
    navigation: Navigation,
    regions: RegionState,
    window_width: f32,
}

impl ShellModel {
    /// A shell on its home page with the default regions (Sidebar open).
    pub fn new(snapshot: ShellSnapshot, window_width: f32) -> Self {
        Self {
            snapshot,
            navigation: Navigation::default(),
            regions: RegionState::default(),
            window_width,
        }
    }

    pub fn snapshot(&self) -> &ShellSnapshot {
        &self.snapshot
    }

    pub fn navigation(&self) -> &Navigation {
        &self.navigation
    }

    pub fn regions(&self) -> &RegionState {
        &self.regions
    }

    pub fn window_width(&self) -> f32 {
        self.window_width
    }

    /// Tracks the window width; regions collapse and reopen as it changes (X1).
    pub fn set_window_width(&mut self, width: f32) {
        self.window_width = width;
    }

    pub fn placement(&self, region: Region) -> Placement {
        self.regions.placement(region, self.window_width)
    }

    /// Whether a region is on screen now, inline or as the open overlay. Full pages hide them all.
    pub fn region_visible(&self, region: Region) -> bool {
        if self.navigation.page.is_full_page() {
            return false;
        }
        match self.placement(region) {
            Placement::Inline => true,
            Placement::Collapsed => self.regions.overlay() == Some(region),
            Placement::Closed => false,
        }
    }

    /// Opens or closes a region. A collapsed region toggles the overlay instead (X1).
    pub fn toggle_region(&mut self, region: Region) {
        self.regions.toggle(region, self.window_width);
    }

    /// The regions shown inline beside the Conversation, left to right.
    pub fn inline_regions(&self) -> Vec<Region> {
        INLINE_ORDER
            .into_iter()
            .filter(|region| {
                self.region_visible(*region) && self.placement(*region) == Placement::Inline
            })
            .collect()
    }

    /// The region open as an overlay, if one is open and on screen.
    pub fn overlay(&self) -> Option<Region> {
        self.regions
            .overlay()
            .filter(|region| self.region_visible(*region))
    }

    pub fn close_overlay(&mut self) {
        self.regions.close_overlay();
    }

    /// The Sidebar toggle: shows or hides the Sidebar, inline or as the overlay.
    pub fn toggle_sidebar(&mut self) {
        self.toggle_region(Region::Sidebar);
    }

    pub fn open_page(&mut self, page: Page) {
        self.navigation.open(page);
    }

    pub fn go_back(&mut self) {
        self.navigation.back();
    }

    pub fn go_forward(&mut self) {
        self.navigation.forward();
    }

    /// The mode button (P6). Returns true when the mode changed.
    pub fn press_mode_switch(&mut self) -> bool {
        self.navigation.press_mode_switch()
    }

    /// The Canvas content for the current page, mode and open session.
    pub fn canvas(&self) -> Canvas {
        Canvas::of(
            self.navigation.page,
            self.navigation.mode,
            self.snapshot.session_title.as_deref(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{DataSource, SampleDataSource};
    use crate::model::navigation::{Mode, Page};

    fn shell(width: f32) -> ShellModel {
        ShellModel::new(SampleDataSource.snapshot(), width)
    }

    #[test]
    fn a_new_shell_is_on_home_with_only_the_sidebar_open() {
        let model = shell(1440.0);
        assert_eq!(model.navigation().page, Page::Home);
        assert!(model.region_visible(Region::Sidebar));
        assert!(!model.region_visible(Region::WorkPanel));
    }

    #[test]
    fn a_narrow_window_collapses_the_sidebar_into_a_closed_overlay() {
        let model = shell(900.0);
        assert_eq!(model.placement(Region::Sidebar), Placement::Collapsed);
        assert!(!model.region_visible(Region::Sidebar));
    }

    #[test]
    fn full_pages_hide_every_region_but_keep_the_user_choice() {
        let mut model = shell(1440.0);
        model.navigation.open(Page::Tasks);
        assert!(!model.region_visible(Region::Sidebar));
        assert!(model.regions().is_open(Region::Sidebar));
    }

    #[test]
    fn inline_regions_keep_the_glossary_order_when_wide() {
        let mut model = shell(1600.0);
        model.toggle_region(Region::WorkPanel);
        model.toggle_region(Region::SessionSummary);
        assert_eq!(
            model.inline_regions(),
            vec![Region::SessionSummary, Region::WorkPanel]
        );
    }

    #[test]
    fn a_collapsed_region_is_the_overlay_until_it_is_closed() {
        let mut model = shell(900.0);
        assert_eq!(model.overlay(), None);
        model.toggle_region(Region::Sidebar);
        assert_eq!(model.overlay(), Some(Region::Sidebar));
        model.close_overlay();
        assert_eq!(model.overlay(), None);
        assert!(model.regions().is_open(Region::Sidebar));
    }

    #[test]
    fn the_canvas_shows_the_session_then_the_full_page_over_the_home() {
        let mut model = shell(1440.0);
        assert_eq!(
            model.canvas(),
            Canvas::Session("Implementation of master catalogs".into())
        );
        model.navigation.open(Page::Tasks);
        assert_eq!(model.canvas(), Canvas::Page(Page::Tasks));
    }

    #[test]
    fn with_no_open_session_the_canvas_is_the_mode_home() {
        let mut snapshot = SampleDataSource.snapshot();
        snapshot.session_title = None;
        let model = ShellModel::new(snapshot, 1440.0);
        assert_eq!(model.canvas(), Canvas::Home(Mode::Workspaces));
    }
}
