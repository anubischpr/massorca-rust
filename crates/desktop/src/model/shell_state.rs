//! The shell's state without GPUI: the page the user is on, the regions they opened, the
//! command palette and the snapshot the views draw. Actions change it through plain methods.

use super::canvas::Canvas;
use super::navigation::{Mode, Navigation, Page};
use super::palette::{Command, PaletteAction, PaletteMode, PaletteModel, palette_items};
use super::palette_recent::RecentActions;
use super::region_cycle::Zone;
use super::regions::{Placement, Region, RegionState};
use super::shell::{ShellSnapshot, find_session};
use super::sidebar::{SidebarContent, sidebar_content};
use super::status_items::{StatusItem, StatusItemVisibility};
use super::teams::{TeamPage, TeamsSidebar, TeamsState, teams_sidebar};

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
    /// The Bot Teams data the Bot Teams pages and Sidebar show (screens 4).
    teams: TeamsState,
    window_width: f32,
    /// Present while the command palette is open.
    palette: Option<PaletteModel>,
    /// The ring whose usage popover is open, by its position in the snapshot.
    usage_popover: Option<usize>,
    /// The Status bar items this window hides.
    status_items: StatusItemVisibility,
    /// The commands and workspaces the palette ran last, for its Recent rows.
    recent: RecentActions,
    /// The workspace the Sidebar opened; its start page shows while no session is open.
    selected_workspace: Option<String>,
}

impl ShellModel {
    /// A shell on its home page with the default regions (Sidebar open).
    pub fn new(snapshot: ShellSnapshot, window_width: f32) -> Self {
        Self {
            snapshot,
            navigation: Navigation::default(),
            regions: RegionState::default(),
            teams: TeamsState::unavailable(),
            window_width,
            palette: None,
            usage_popover: None,
            status_items: StatusItemVisibility::default(),
            recent: RecentActions::default(),
            selected_workspace: None,
        }
    }

    pub fn snapshot(&self) -> &ShellSnapshot {
        &self.snapshot
    }

    /// Swaps in a new snapshot from the store. The open session stays open while it still exists.
    pub fn set_snapshot(&mut self, mut snapshot: ShellSnapshot) {
        let open = self
            .snapshot
            .active_session
            .as_deref()
            .and_then(|id| find_session(&snapshot.projects, id))
            .map(|session| (session.title.clone(), session.id.clone()));
        if let Some((title, id)) = open {
            snapshot.session_title = Some(title);
            snapshot.active_session = Some(id);
        }
        self.snapshot = snapshot;
    }

    pub fn navigation(&self) -> &Navigation {
        &self.navigation
    }

    pub fn regions(&self) -> &RegionState {
        &self.regions
    }

    pub fn teams(&self) -> &TeamsState {
        &self.teams
    }

    pub fn teams_mut(&mut self) -> &mut TeamsState {
        &mut self.teams
    }

    /// Swaps in the Bot Teams data of a source; the shell starts with the source it was given.
    pub fn set_teams(&mut self, teams: TeamsState) {
        self.teams = teams;
    }

    /// The Bot Teams Sidebar for the page on screen.
    pub fn bot_teams_sidebar(&self) -> TeamsSidebar {
        teams_sidebar(&self.teams, self.navigation.page)
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
        // Bot Teams has no Workspaces regions: the Sidebar is the only one on screen there.
        if self.navigation.mode == Mode::BotTeams && region != Region::Sidebar {
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

    /// Whether a region is recorded as the open overlay, on screen or not.
    pub fn has_overlay(&self) -> bool {
        self.regions.overlay().is_some()
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
        self.teams.cancel_pick();
        self.navigation.open(page);
    }

    /// Opens a Bot Teams page and makes its team the one the Sidebar lists projects for.
    pub fn open_team_page(&mut self, page: TeamPage) {
        self.teams.cancel_pick();
        self.teams.select(page.team());
        self.navigation.open(Page::Team(page));
    }

    pub fn go_back(&mut self) {
        self.navigation.back();
    }

    pub fn go_forward(&mut self) {
        self.navigation.forward();
    }

    /// The mode button (P6). Returns true when the mode changed.
    pub fn press_mode_switch(&mut self) -> bool {
        self.teams.cancel_pick();
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

    /// The Canvas content for the current page, mode and open session. A selected workspace shows
    /// its start page on the Workspaces home until a session opens.
    pub fn canvas(&self) -> Canvas {
        let on_start_page = self.navigation.page == Page::Home
            && self.navigation.mode == Mode::Workspaces
            && self.snapshot.session_title.is_none();
        if let (true, Some(id)) = (on_start_page, &self.selected_workspace) {
            return Canvas::Workspace(id.clone());
        }
        Canvas::of(
            self.navigation.page,
            self.navigation.mode,
            self.snapshot.session_title.as_deref(),
        )
    }

    /// The workspace the Sidebar or the start page chose, if any.
    pub fn selected_workspace(&self) -> Option<&str> {
        self.selected_workspace.as_deref()
    }

    /// A click on a workspace opens its start page, leaving any full page first (D-WS-01).
    pub fn open_workspace(&mut self, id: impl Into<String>) {
        self.selected_workspace = Some(id.into());
        self.navigation.open(Page::Home);
    }

    /// Chooses the workspace whose start page shows, or none for the start page with no workspace.
    pub fn select_workspace(&mut self, id: Option<String>) {
        self.selected_workspace = id;
        self.navigation.open(Page::Home);
    }

    /// The zones F6 cycles through now: the frame's parts, and the regions on screen.
    pub fn focus_zones(&self) -> Vec<Zone> {
        Zone::ORDER
            .into_iter()
            .filter(|zone| match zone {
                Zone::Sidebar => self.sidebar_inline() || self.sidebar_overlay(),
                Zone::SessionSummary => self.region_visible(Region::SessionSummary),
                Zone::WorkPanel => self.region_visible(Region::WorkPanel),
                Zone::ProjectSidebar => self.region_visible(Region::ProjectSidebar),
                Zone::TitleBar | Zone::Rail | Zone::Conversation | Zone::StatusBar => true,
            })
            .collect()
    }

    /// Which Status bar items this window shows (StatusBar spec).
    pub fn status_items(&self) -> &StatusItemVisibility {
        &self.status_items
    }

    /// Shows a hidden Status bar item or hides a shown one, as the visibility menu does.
    pub fn toggle_status_item(&mut self, item: StatusItem) {
        self.status_items.toggle(item);
    }

    /// The ring whose usage popover is open, if it still exists in the snapshot.
    pub fn usage_popover(&self) -> Option<usize> {
        self.usage_popover
            .filter(|&position| position < self.snapshot.usage.len())
    }

    /// Opens a ring's usage popover, or closes it when that one is open (D-SH-15). Only one is
    /// open at a time.
    pub fn toggle_usage_popover(&mut self, position: usize) {
        self.usage_popover = match self.usage_popover() {
            Some(open) if open == position => None,
            _ => Some(position),
        };
    }

    pub fn close_usage_popover(&mut self) {
        self.usage_popover = None;
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
            self.open_palette(PaletteMode::Commands);
        }
    }

    /// Opens the palette in a mode over the current snapshot. The commands come with their Recent
    /// rows; "Go to file" has no files to list yet (D-CP-02).
    pub fn open_palette(&mut self, mode: PaletteMode) {
        let items = match mode {
            PaletteMode::Commands => {
                let all = palette_items(&self.snapshot);
                let mut items = self.recent.rows(&all);
                items.extend(all);
                items
            }
            PaletteMode::GoToFile => Vec::new(),
        };
        self.palette = Some(PaletteModel::new(items).with_mode(mode));
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
        self.recent.record(&action);
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
    use crate::model::teams::{SampleTeamsSource, TeamId, TeamPage, TeamsSource};

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
    fn bot_teams_keeps_only_the_sidebar_and_shows_no_workspaces_session() {
        let mut model = shell(1440.0);
        model.set_teams(SampleTeamsSource.teams());
        assert!(model.press_mode_switch());
        assert_eq!(model.canvas(), Canvas::Home(Mode::BotTeams));
        assert!(model.region_visible(Region::Sidebar));
        assert!(!model.region_visible(Region::WorkPanel));
        assert!(model.inline_regions().is_empty());
    }

    #[test]
    fn a_team_page_takes_the_canvas_selects_its_team_and_keeps_the_sidebar() {
        let mut model = shell(1440.0);
        model.set_teams(SampleTeamsSource.teams());
        model.press_mode_switch();
        model.open_team_page(TeamPage::Board(TeamId(1)));
        assert_eq!(model.canvas(), Canvas::Team(TeamPage::Board(TeamId(1))));
        assert!(model.sidebar_inline());
        assert_eq!(model.teams().selected(), Some(TeamId(1)));
    }

    #[test]
    fn the_sidebar_lists_the_content_of_the_current_mode() {
        let mut model = shell(1440.0);
        assert!(matches!(model.sidebar(), SidebarContent::Workspaces { .. }));
        assert!(model.press_mode_switch());
        assert!(matches!(model.sidebar(), SidebarContent::BotTeams));
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
    fn the_f6_cycle_skips_hidden_regions_and_joins_an_open_one() {
        let mut model = shell(1440.0);
        assert_eq!(
            model.focus_zones(),
            vec![
                Zone::TitleBar,
                Zone::Rail,
                Zone::Sidebar,
                Zone::Conversation,
                Zone::StatusBar,
            ]
        );
        model.toggle_region(Region::WorkPanel);
        assert_eq!(
            model.focus_zones()[3..6],
            [Zone::Conversation, Zone::WorkPanel, Zone::StatusBar]
        );
        model.open_page(Page::Tasks);
        assert_eq!(
            model.focus_zones(),
            vec![
                Zone::TitleBar,
                Zone::Rail,
                Zone::Conversation,
                Zone::StatusBar
            ]
        );
    }

    #[test]
    fn a_new_snapshot_keeps_the_open_session_while_it_still_exists() {
        let mut model = shell(1440.0);
        model.open_session("s1a");
        let mut fresh = SampleDataSource.snapshot();
        fresh.session_title = None;
        fresh.active_session = None;
        model.set_snapshot(fresh.clone());
        assert_eq!(model.snapshot().active_session.as_deref(), Some("s1a"));
        fresh.projects.clear();
        model.set_snapshot(fresh);
        assert_eq!(model.snapshot().active_session, None, "the session is gone");
        assert!(model.snapshot().projects.is_empty());
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
