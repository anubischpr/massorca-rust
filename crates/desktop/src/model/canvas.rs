//! What the Canvas shows: the mode's home, an open session, a team page or a full page (A8,
//! D-SH-01).

use super::navigation::{Mode, Page};
use super::teams::TeamPage;

/// The content of the Canvas, decided by the page, the mode and the open session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Canvas {
    /// The mode's home with no session open: the start page, or the Bot Teams team list (P13).
    Home(Mode),
    /// An open Workspaces session, by its title. Its live terminal is the view; the conversation comes later.
    Session(String),
    /// A Bot Teams page, which keeps the Sidebar (screens 4).
    Team(TeamPage),
    /// A full page, which covers the Canvas and hides every region (A8).
    Page(Page),
    /// A workspace's start page, with no session open (D-AG-01). Its id is the workspace's.
    Workspace(String),
}

impl Canvas {
    /// A team page wins; then a full page; then, in Workspaces, an open session; then the mode's
    /// home. Bot Teams never shows a Workspaces session.
    pub fn of(page: Page, mode: Mode, session_title: Option<&str>) -> Self {
        match page {
            Page::Team(team) => Canvas::Team(team),
            _ if page.is_full_page() => Canvas::Page(page),
            _ => match (mode, session_title) {
                (Mode::Workspaces, Some(title)) => Canvas::Session(title.to_owned()),
                _ => Canvas::Home(mode),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::teams::TeamId;
    use super::*;

    #[test]
    fn with_no_session_the_canvas_shows_the_mode_home() {
        assert_eq!(
            Canvas::of(Page::Home, Mode::Workspaces, None),
            Canvas::Home(Mode::Workspaces)
        );
        assert_eq!(
            Canvas::of(Page::Home, Mode::BotTeams, None),
            Canvas::Home(Mode::BotTeams)
        );
    }

    #[test]
    fn an_open_session_replaces_the_workspaces_home_and_a_full_page_replaces_both() {
        assert_eq!(
            Canvas::of(Page::Home, Mode::Workspaces, Some("Feed")),
            Canvas::Session("Feed".into())
        );
        assert_eq!(
            Canvas::of(Page::Tasks, Mode::Workspaces, Some("Feed")),
            Canvas::Page(Page::Tasks)
        );
    }

    #[test]
    fn bot_teams_never_shows_a_workspaces_session() {
        assert_eq!(
            Canvas::of(Page::Home, Mode::BotTeams, Some("Feed")),
            Canvas::Home(Mode::BotTeams)
        );
    }

    #[test]
    fn a_team_page_takes_the_canvas_and_keeps_the_sidebar_open() {
        let board = TeamPage::Board(TeamId(1));
        assert_eq!(
            Canvas::of(Page::Team(board), Mode::BotTeams, None),
            Canvas::Team(board)
        );
        assert!(!Page::Team(board).is_full_page());
    }
}
