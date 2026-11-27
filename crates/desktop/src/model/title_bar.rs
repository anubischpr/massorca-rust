//! The Title bar's content: navigation, the title, the Sidebar toggle and the region toggles (D-SH-03).
//! Each page draws its Back control as its artboard does: sessions show Back and Forward, full pages
//! one Back, and Bot Teams pages none.

use super::navigation::{Mode, Page};
use super::regions::Region;
use super::shell::SessionRow;
use super::shell_state::ShellModel;
use super::teams::TeamPage;

/// The title when no session is open (the landing view of D-SH-03).
pub const NO_SESSION_TITLE: &str = "No session open";

/// The title of the Bot Teams home, the team list (screens 4.5).
pub const BOT_TEAMS_TITLE: &str = "Bot teams";

/// Joins a team or a marketplace name to its page, as the artboards read: "Desarrollo · Board".
const TITLE_SEPARATOR: &str = " · ";

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

/// A session's terminal and the workspace it runs in: what "Open in Work panel" reveals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionTerminal {
    pub workspace: String,
    pub terminal: String,
}

/// The Back control at the start of the Title bar, as the artboards draw each kind of page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackControl {
    /// No Back. Bot Teams pages keep the Sidebar toggle and name their team (bt-board).
    None,
    /// Back and Forward icons beside the Sidebar toggle, for a session (sh-empty-app).
    History,
    /// One icon Back beside the title, on a full page (bt-settings-appearance, bt-calendar-week).
    Icon,
    /// One labelled Back beside the title, on a full page (ws-tasks, ws-automations).
    Labelled,
}

/// What the Title bar shows right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TitleBarModel {
    /// The full page on screen, if any; its Back control and title replace the session.
    pub full_page: Option<Page>,
    pub back: BackControl,
    pub title: String,
    /// No session is open, so the title reads in tertiary text.
    pub title_is_placeholder: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    /// The Sidebar is on screen.
    pub sidebar_on: bool,
    /// Names the action the Sidebar toggle takes.
    pub sidebar_label: &'static str,
    /// Full pages hide the Sidebar and its toggle, as their artboards do.
    pub show_sidebar_toggle: bool,
    /// Empty on full pages.
    pub region_toggles: Vec<RegionToggle>,
    /// New chat: the start page of the current workspace, or of none. Off on full pages and Bot Teams pages.
    pub new_chat: bool,
    /// Terminal: a new terminal in this workspace's Work panel. None without a workspace or a service.
    pub new_terminal_in: Option<String>,
    /// Open in Work panel: the open session's terminal. None without a session or for a chat session.
    pub session_terminal: Option<SessionTerminal>,
}

/// The Title bar for the shell's current state.
pub fn title_bar(model: &ShellModel) -> TitleBarModel {
    let navigation = model.navigation();
    let full_page = navigation.page.is_full_page().then_some(navigation.page);
    let bot_teams = navigation.mode == Mode::BotTeams;
    let session = model.snapshot().session_title.as_ref();
    let (title, title_is_placeholder) = match (full_page, navigation.page, session) {
        (Some(page), _, _) => (full_page_title(model, page), false),
        (None, Page::Team(page), _) => (team_page_title(model, page), false),
        (None, _, _) if bot_teams => (BOT_TEAMS_TITLE.to_owned(), false),
        (None, _, Some(session)) => (session.clone(), false),
        (None, _, None) => (NO_SESSION_TITLE.to_owned(), true),
    };
    let back = match full_page {
        Some(page) => full_page_back(page),
        None if bot_teams => BackControl::None,
        None => BackControl::History,
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
    // The actions sit in the right group with the region toggles: on the Workspaces home and sessions only.
    let right_group = full_page.is_none() && !bot_teams;
    TitleBarModel {
        full_page,
        back,
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
        show_sidebar_toggle: full_page.is_none(),
        region_toggles,
        new_chat: right_group,
        new_terminal_in: model
            .current_workspace()
            .filter(|_| right_group && model.snapshot().service_status.is_some())
            .map(str::to_owned),
        session_terminal: model
            .open_session_row()
            .filter(|_| right_group)
            .and_then(session_terminal_of),
    }
}

/// The terminal of a session the Work panel can show; a chat session has none.
fn session_terminal_of(session: &SessionRow) -> Option<SessionTerminal> {
    Some(SessionTerminal {
        workspace: session.workspace_id.clone(),
        terminal: session.terminal_id.clone()?,
    })
}

/// The Back control of a full page. Tasks and Automations draw a labelled Back; the other full
/// pages draw one icon Back, as their artboards do.
fn full_page_back(page: Page) -> BackControl {
    match page {
        Page::Tasks | Page::Automations => BackControl::Labelled,
        _ => BackControl::Icon,
    }
}

/// A full page is named by its page; a Marketplace item page by the item's name.
fn full_page_title(model: &ShellModel, page: Page) -> String {
    match page {
        Page::Marketplace(super::navigation::MarketplacePage::Item(id)) => {
            model.marketplace().item(id).map_or_else(
                || page.title().to_owned(),
                |item| format!("{}{TITLE_SEPARATOR}{}", page.title(), item.name),
            )
        }
        _ => page.title().to_owned(),
    }
}

/// A team page is named by its team and page, and a project's page by the team and the project's
/// name: "Desarrollo · Board" (bt-board).
fn team_page_title(model: &ShellModel, page: TeamPage) -> String {
    let page_name = match page {
        TeamPage::Project(team, project) => model
            .teams()
            .team(team)
            .and_then(|team| team.project(project))
            .map_or_else(|| page.title().to_owned(), |project| project.name.clone()),
        _ => page.title().to_owned(),
    };
    match model.teams().team(page.team()) {
        Some(team) => format!("{}{TITLE_SEPARATOR}{page_name}", team.name),
        None => page_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{DataSource, SampleDataSource};
    use crate::model::marketplace::{MarketplaceSource, SampleMarketplaceSource};
    use crate::model::navigation::MarketplacePage;
    use crate::model::shell::{ServiceStatus, ServiceTone, ShellSnapshot};
    use crate::model::teams::{ProjectId, SampleTeamsSource, TeamId, TeamPage, TeamsSource};

    fn shell() -> ShellModel {
        ShellModel::new(SampleDataSource.snapshot(), 1440.0)
    }

    fn bot_teams_shell() -> ShellModel {
        let mut model = shell();
        model.set_teams(SampleTeamsSource.teams());
        model.press_mode_switch();
        model
    }

    #[test]
    fn a_session_titles_the_bar_and_draws_back_and_forward() {
        let bar = title_bar(&shell());
        assert_eq!(bar.title, "Implementation of master catalogs");
        assert!(!bar.title_is_placeholder);
        assert!(!bar.can_go_back && !bar.can_go_forward);
        assert_eq!(bar.full_page, None);
        assert_eq!(bar.back, BackControl::History);
        assert!(bar.show_sidebar_toggle);
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
    fn a_full_page_shows_its_title_and_hides_the_sidebar_and_region_toggles() {
        let mut model = shell();
        model.open_page(Page::Tasks);
        let bar = title_bar(&model);
        assert_eq!(bar.full_page, Some(Page::Tasks));
        assert_eq!(bar.title, "Tasks");
        assert!(bar.region_toggles.is_empty());
        assert!(!bar.show_sidebar_toggle);
        assert!(bar.can_go_back);
    }

    #[test]
    fn each_full_page_draws_back_as_its_artboard_does() {
        let cases = [
            // ws-tasks and ws-automations draw a labelled Back.
            (Page::Tasks, BackControl::Labelled),
            (Page::Automations, BackControl::Labelled),
            // bt-settings-appearance, bt-calendar-week, bt-marketplace-catalogue and
            // sh-setup-checklist draw one icon Back. Pairing has no artboard and follows them.
            (Page::Settings, BackControl::Icon),
            (Page::Calendar, BackControl::Icon),
            (
                Page::Marketplace(MarketplacePage::Discover),
                BackControl::Icon,
            ),
            (Page::Setup, BackControl::Icon),
            (Page::Pairing, BackControl::Icon),
        ];
        for (page, back) in cases {
            let mut model = shell();
            model.open_page(page);
            assert_eq!(title_bar(&model).back, back, "{page:?}");
        }
    }

    #[test]
    fn a_marketplace_item_page_names_the_marketplace_and_the_item() {
        let mut model = shell();
        model.set_marketplace(SampleMarketplaceSource::default().snapshot());
        let item = model.marketplace().items[0].clone();
        model.open_page(Page::Marketplace(MarketplacePage::Item(item.id)));
        assert_eq!(
            title_bar(&model).title,
            format!("Marketplace · {}", item.name)
        );
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

    #[test]
    fn a_bot_teams_page_is_named_by_its_team_and_page() {
        let mut model = bot_teams_shell();
        assert_eq!(title_bar(&model).title, BOT_TEAMS_TITLE);
        model.open_team_page(TeamPage::Board(TeamId(1)));
        assert_eq!(title_bar(&model).title, "Desarrollo · Board");
        model.open_team_page(TeamPage::Conference(TeamId(1)));
        assert_eq!(title_bar(&model).title, "Desarrollo · Conference room");
        model.open_team_page(TeamPage::Routines(TeamId(1)));
        assert_eq!(title_bar(&model).title, "Desarrollo · Routines");
        model.open_team_page(TeamPage::NewRoutine(TeamId(1)));
        assert_eq!(title_bar(&model).title, "Desarrollo · New routine");
    }

    #[test]
    fn a_team_project_page_is_named_by_its_team_and_project() {
        let mut model = bot_teams_shell();
        model.open_team_page(TeamPage::Project(TeamId(1), ProjectId(1)));
        assert_eq!(
            title_bar(&model).title,
            "Desarrollo · Desarrollo onboarding"
        );
    }

    #[test]
    fn bot_teams_pages_draw_no_back_and_keep_the_sidebar_toggle() {
        let mut model = bot_teams_shell();
        let home = title_bar(&model);
        assert_eq!(home.back, BackControl::None);
        assert!(home.show_sidebar_toggle);
        assert!(home.region_toggles.is_empty());
        model.open_team_page(TeamPage::Board(TeamId(1)));
        let board = title_bar(&model);
        assert_eq!(board.back, BackControl::None);
        assert_eq!(board.full_page, None);
        assert!(board.show_sidebar_toggle);
        assert!(board.region_toggles.is_empty());
    }

    fn connected() -> ServiceStatus {
        ServiceStatus {
            tone: ServiceTone::Ok,
            text: "Service".to_owned(),
            tooltip: "Connected".to_owned(),
        }
    }

    /// Gives the sample session `id` a terminal, wherever it sits in the tree.
    fn with_terminal(snapshot: &mut ShellSnapshot, id: &str, terminal: &str) {
        fn walk(sessions: &mut [SessionRow], id: &str, terminal: &str) {
            for session in sessions {
                if session.id == id {
                    session.terminal_id = Some(terminal.to_owned());
                }
                walk(&mut session.children, id, terminal);
            }
        }
        for project in &mut snapshot.projects {
            for host in &mut project.hosts {
                walk(&mut host.sessions, id, terminal);
            }
        }
    }

    #[test]
    fn new_chat_is_offered_on_the_workspaces_home_and_sessions_only() {
        let mut model = shell();
        assert!(title_bar(&model).new_chat, "a session");
        model.close_session();
        assert!(
            title_bar(&model).new_chat,
            "the start page with no workspace"
        );
        model.open_page(Page::Tasks);
        assert!(!title_bar(&model).new_chat, "a full page");
        assert!(!title_bar(&bot_teams_shell()).new_chat, "Bot Teams");
    }

    #[test]
    fn a_terminal_needs_a_workspace_and_a_service() {
        let mut model = shell();
        assert_eq!(title_bar(&model).new_terminal_in, None, "no service");
        let mut snapshot = SampleDataSource.snapshot();
        snapshot.service_status = Some(connected());
        model.set_snapshot(snapshot);
        assert_eq!(title_bar(&model).new_terminal_in.as_deref(), Some("w1"));
        model.close_session();
        model.select_workspace(None);
        assert_eq!(
            title_bar(&model).new_terminal_in,
            None,
            "the start page with no workspace"
        );
    }

    #[test]
    fn open_in_work_panel_reveals_the_open_sessions_terminal() {
        let mut model = shell();
        assert_eq!(
            title_bar(&model).session_terminal,
            None,
            "a session without a terminal"
        );
        let mut snapshot = SampleDataSource.snapshot();
        with_terminal(&mut snapshot, "s1", "t-run");
        model.set_snapshot(snapshot);
        assert_eq!(
            title_bar(&model).session_terminal,
            Some(SessionTerminal {
                workspace: "w1".to_owned(),
                terminal: "t-run".to_owned(),
            })
        );
        model.close_session();
        assert_eq!(title_bar(&model).session_terminal, None, "no session open");
    }
}
