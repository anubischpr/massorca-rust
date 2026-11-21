//! The Title bar's content: navigation, the title, the Sidebar toggle and the region
//! toggles (D-SH-03). Full pages swap Back and the title in, and hide the region toggles.

use super::navigation::{Mode, Page};
use super::regions::Region;
use super::shell_state::ShellModel;
use super::teams::TeamPage;

/// The title when no session is open (the landing view of D-SH-03).
pub const NO_SESSION_TITLE: &str = "No session open";

/// The title of the Bot Teams home, the team list (screens 4.5).
pub const BOT_TEAMS_TITLE: &str = "Bot teams";

/// The regions with a Title bar toggle, in D-SH-03 order.
const TOGGLED_REGIONS: [Region; 3] = [
    Region::SessionSummary,
    Region::WorkPanel,
    Region::ProjectSidebar,
];

/// One region toggle: on while its region is on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionToggle {
    pub region: Region,
    pub on: bool,
}

/// What the Title bar shows right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TitleBarModel {
    /// The full page on screen, if any; its Back button and title replace the session.
    pub full_page: Option<Page>,
    pub title: String,
    /// No session is open, so the title reads in tertiary text.
    pub title_is_placeholder: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    /// The Sidebar is on screen.
    pub sidebar_on: bool,
    /// Names the action the Sidebar toggle takes.
    pub sidebar_label: &'static str,
    /// Off on full pages, where the Sidebar is hidden.
    pub sidebar_enabled: bool,
    /// Empty on full pages.
    pub region_toggles: Vec<RegionToggle>,
}

/// The Title bar for the shell's current state.
pub fn title_bar(model: &ShellModel) -> TitleBarModel {
    let navigation = model.navigation();
    let full_page = navigation.page.is_full_page().then_some(navigation.page);
    let bot_teams = navigation.mode == Mode::BotTeams;
    let session = model.snapshot().session_title.as_ref();
    let (title, title_is_placeholder) = match (full_page, navigation.page, session) {
        (Some(page), _, _) => (page.title().to_owned(), false),
        (None, Page::Team(page), _) => (team_page_title(model, page), false),
        (None, _, _) if bot_teams => (BOT_TEAMS_TITLE.to_owned(), false),
        (None, _, Some(session)) => (session.clone(), false),
        (None, _, None) => (NO_SESSION_TITLE.to_owned(), true),
    };
    // Bot Teams pages have no Workspaces regions to toggle.
    let region_toggles = if full_page.is_some() || bot_teams {
        Vec::new()
    } else {
        TOGGLED_REGIONS
            .into_iter()
            .map(|region| RegionToggle {
                region,
                on: model.region_visible(region),
            })
            .collect()
    };
    let sidebar_on = model.region_visible(Region::Sidebar);
    TitleBarModel {
        full_page,
        title,
        title_is_placeholder,
        can_go_back: navigation.can_go_back(),
        can_go_forward: navigation.can_go_forward(),
        sidebar_on,
        sidebar_label: if sidebar_on {
            "Hide sidebar"
        } else {
            "Show sidebar"
        },
        sidebar_enabled: full_page.is_none(),
        region_toggles,
    }
}

/// A team page is named by its page, and a project's page by the project's name.
fn team_page_title(model: &ShellModel, page: TeamPage) -> String {
    match page {
        TeamPage::Project(team, project) => model
            .teams()
            .team(team)
            .and_then(|team| team.project(project))
            .map_or_else(|| page.title().to_owned(), |project| project.name.clone()),
        _ => page.title().to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{DataSource, SampleDataSource};

    fn shell() -> ShellModel {
        ShellModel::new(SampleDataSource.snapshot(), 1440.0)
    }

    #[test]
    fn a_session_titles_the_bar_and_back_is_off_without_history() {
        let bar = title_bar(&shell());
        assert_eq!(bar.title, "Implementation of master catalogs");
        assert!(!bar.title_is_placeholder);
        assert!(!bar.can_go_back && !bar.can_go_forward);
        assert_eq!(bar.full_page, None);
        assert_eq!(bar.region_toggles.len(), 3);
    }

    #[test]
    fn without_a_session_the_bar_says_no_session_open() {
        let mut snapshot = SampleDataSource.snapshot();
        snapshot.session_title = None;
        let bar = title_bar(&ShellModel::new(snapshot, 1440.0));
        assert_eq!(bar.title, NO_SESSION_TITLE);
        assert!(bar.title_is_placeholder);
    }

    #[test]
    fn a_full_page_shows_its_title_and_hides_the_region_toggles() {
        let mut model = shell();
        model.open_page(Page::Tasks);
        let bar = title_bar(&model);
        assert_eq!(bar.full_page, Some(Page::Tasks));
        assert_eq!(bar.title, "Tasks");
        assert!(bar.region_toggles.is_empty());
        assert!(!bar.sidebar_enabled);
        assert!(bar.can_go_back);
    }

    #[test]
    fn region_toggles_show_what_is_on_screen() {
        let mut model = shell();
        model.toggle_region(Region::WorkPanel);
        let bar = title_bar(&model);
        let on = |region| {
            bar.region_toggles
                .iter()
                .find(|toggle| toggle.region == region)
                .map(|toggle| toggle.on)
        };
        assert_eq!(on(Region::WorkPanel), Some(true));
        assert_eq!(on(Region::SessionSummary), Some(false));
        assert!(bar.sidebar_on);
        assert_eq!(bar.sidebar_label, "Hide sidebar");
    }

    #[test]
    fn the_sidebar_toggle_offers_to_show_it_again() {
        let mut model = shell();
        model.toggle_sidebar();
        let bar = title_bar(&model);
        assert!(!bar.sidebar_on);
        assert_eq!(bar.sidebar_label, "Show sidebar");
    }
}
