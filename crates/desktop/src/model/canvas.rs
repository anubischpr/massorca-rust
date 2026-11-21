//! What the Canvas shows: the mode's home, an open session or a full page (A8, D-SH-01).

use super::navigation::{Mode, Page};

/// The content of the Canvas, decided by the page, the mode and the open session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Canvas {
    /// The mode's home with no session open: the start page, or the Bot Teams team list (P13).
    Home(Mode),
    /// An open session, by its title. Its conversation is not built yet.
    Session(String),
    /// A full page, which covers the Canvas and hides every region (A8).
    Page(Page),
}

impl Canvas {
    /// A full page wins, then an open session, then the mode's home.
    pub fn of(page: Page, mode: Mode, session_title: Option<&str>) -> Self {
        match (page.is_full_page(), session_title) {
            (true, _) => Canvas::Page(page),
            (false, Some(title)) => Canvas::Session(title.to_owned()),
            (false, None) => Canvas::Home(mode),
        }
    }
}

#[cfg(test)]
mod tests {
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
    fn an_open_session_replaces_the_home_and_a_full_page_replaces_both() {
        assert_eq!(
            Canvas::of(Page::Home, Mode::Workspaces, Some("Feed")),
            Canvas::Session("Feed".into())
        );
        assert_eq!(
            Canvas::of(Page::Tasks, Mode::Workspaces, Some("Feed")),
            Canvas::Page(Page::Tasks)
        );
    }
}
