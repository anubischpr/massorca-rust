//! The shell's state without GPUI: the page the user is on, the regions they opened
//! and the snapshot the views draw. Actions change it through plain methods.

use super::navigation::Navigation;
use super::regions::{Placement, Region, RegionState};
use super::shell::ShellSnapshot;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{DataSource, SampleDataSource};
    use crate::model::navigation::Page;

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
}
