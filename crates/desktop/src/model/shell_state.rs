//! The shell's state without GPUI: the page the user is on, the regions they opened, the
//! command palette and the snapshot the views draw. Actions change it through plain methods.

use super::canvas::Canvas;
use super::navigation::{Navigation, Page};
use super::palette::{Command, PaletteAction, PaletteModel, palette_items};
use super::regions::{Placement, Region, RegionState};
use super::shell::{ShellSnapshot, find_session};
use super::sidebar::{SidebarContent, sidebar_content};

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
    /// Present while the command palette is open.
    palette: Option<PaletteModel>,
}

impl ShellModel {
    /// A shell on its home page with the default regions (Sidebar open).
    pub fn new(snapshot: ShellSnapshot, window_width: f32) -> Self {
        Self {
            snapshot,
            navigation: Navigation::default(),
            regions: RegionState::default(),
            window_width,
            palette: None,
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

    /// The Sidebar beside the Canvas, when the window is wide enough and it is open.
    pub fn sidebar_inline(&self) -> bool {
        self.region_visible(Region::Sidebar) && self.placement(Region::Sidebar) == Placement::Inline
    }

    /// The Sidebar as the overlay over the Canvas, when the window is narrow.
    pub fn sidebar_overlay(&self) -> bool {
        self.overlay() == Some(Region::Sidebar)
    }

    /// What the Sidebar lists in the current mode.
    pub fn sidebar(&self) -> SidebarContent {
        sidebar_content(&self.snapshot, self.navigation.mode)
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

    /// Makes a session the open one: the title bar and the Sidebar selection follow it.
    pub fn open_session(&mut self, id: &str) {
        let found = find_session(&self.snapshot.projects, id)
            .map(|session| (session.title.clone(), session.id.clone()));
        if let Some((title, id)) = found {
            self.snapshot.session_title = Some(title);
            self.snapshot.active_session = Some(id);
        }
    }

    /// The Canvas content for the current page, mode and open session.
    pub fn canvas(&self) -> Canvas {
        Canvas::of(
            self.navigation.page,
            self.navigation.mode,
            self.snapshot.session_title.as_deref(),
        )
    }

    /// The palette while it is open.
    pub fn palette(&self) -> Option<&PaletteModel> {
        self.palette.as_ref()
    }

    /// Opens the palette over the current snapshot, or closes it when it is open (Mod+K).
    pub fn toggle_palette(&mut self) {
        if self.palette.is_some() {
            self.palette = None;
        } else {
            self.palette = Some(PaletteModel::new(palette_items(&self.snapshot)));
        }
    }

    pub fn close_palette(&mut self) {
        self.palette = None;
    }

    pub fn set_palette_query(&mut self, query: &str) {
        if let Some(palette) = &mut self.palette {
            palette.set_query(query);
        }
    }

    pub fn move_palette(&mut self, delta: isize) {
        if let Some(palette) = &mut self.palette {
            palette.move_active(delta);
        }
    }

    pub fn select_palette_row(&mut self, index: usize) {
        if let Some(palette) = &mut self.palette {
            palette.select(index);
        }
    }

    /// Enter: closes the palette and returns the active row's action. A disabled row keeps the
    /// palette open and returns nothing.
    pub fn confirm_palette(&mut self) -> Option<PaletteAction> {
        let action = self.palette.as_ref()?.confirm()?.clone();
        self.palette = None;
        Some(action)
    }

    /// Runs what a palette row does. A workspace page is not built yet, so its row only closes
    /// the palette.
    pub fn run_palette_action(&mut self, action: &PaletteAction) {
        match action {
            PaletteAction::OpenWorkspace(_) => {}
            PaletteAction::OpenSession(id) => self.open_session(id),
            PaletteAction::Command(command) => self.run_command(*command),
        }
    }

    /// Runs a command that changes the shell's own state. The theme is app-wide and flips in
    /// the window layer, so it does nothing here.
    pub fn run_command(&mut self, command: Command) {
        match command {
            Command::ToggleTheme => {}
            Command::Open(page) => self.open_page(page),
            Command::Toggle(region) => self.toggle_region(region),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{DataSource, SampleDataSource};
    use crate::model::navigation::Mode;

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
    fn the_sidebar_is_inline_when_wide_and_an_overlay_when_narrow() {
        let mut wide = shell(1440.0);
        assert!(wide.sidebar_inline() && !wide.sidebar_overlay());
        wide.toggle_sidebar();
        assert!(!wide.sidebar_inline());

        let mut narrow = shell(900.0);
        assert!(!narrow.sidebar_inline() && !narrow.sidebar_overlay());
        narrow.toggle_sidebar();
        assert!(narrow.sidebar_overlay() && !narrow.sidebar_inline());
    }

    #[test]
    fn the_sidebar_lists_the_content_of_the_current_mode() {
        let mut model = shell(1440.0);
        assert!(matches!(model.sidebar(), SidebarContent::Workspaces { .. }));
        assert!(model.press_mode_switch());
        assert!(matches!(model.sidebar(), SidebarContent::BotTeams { .. }));
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

    #[test]
    fn mod_k_toggles_the_palette_over_the_snapshot() {
        let mut model = shell(1440.0);
        model.toggle_palette();
        assert!(model.palette().is_some());
        model.toggle_palette();
        assert!(model.palette().is_none());
    }

    #[test]
    fn a_command_row_runs_and_closes_the_palette() {
        let mut model = shell(1440.0);
        model.toggle_palette();
        model.set_palette_query("work panel");
        let action = model.confirm_palette().expect("the matching command runs");
        assert_eq!(
            action,
            PaletteAction::Command(Command::Toggle(Region::WorkPanel))
        );
        model.run_palette_action(&action);
        assert!(model.palette().is_none());
        assert_eq!(model.inline_regions(), vec![Region::WorkPanel]);
    }

    #[test]
    fn a_session_row_opens_that_session_in_the_title_and_the_sidebar() {
        let mut model = shell(1440.0);
        model.toggle_palette();
        model.set_palette_query("feed endpoints");
        let action = model.confirm_palette().expect("the session row runs");
        model.run_palette_action(&action);
        assert_eq!(
            model.snapshot().session_title.as_deref(),
            Some("Feed endpoints")
        );
        assert_eq!(model.snapshot().active_session.as_deref(), Some("s1a"));
    }
}
