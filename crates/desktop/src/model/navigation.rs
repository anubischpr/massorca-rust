//! Modes and pages: the Activity rail and the mode switch (D-SH-09, D-SH-10).

use super::teams::TeamPage;

/// The two modes of the app; each has its own Sidebar and home.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Workspaces,
    BotTeams,
}

impl Mode {
    pub fn other(self) -> Mode {
        match self {
            Mode::Workspaces => Mode::BotTeams,
            Mode::BotTeams => Mode::Workspaces,
        }
    }

    /// Accessible name of the mode button ("Workspaces mode").
    pub fn label(self) -> &'static str {
        match self {
            Mode::Workspaces => "Workspaces mode",
            Mode::BotTeams => "Bot Teams mode",
        }
    }

    /// Tooltip of the mode button: it names the target.
    pub fn switch_tooltip(self) -> &'static str {
        match self.other() {
            Mode::Workspaces => "Switch to Workspaces",
            Mode::BotTeams => "Switch to Bot Teams",
        }
    }
}

/// What the Canvas shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    /// The mode's home: start page (Workspaces) or team list (Bot Teams), P13.
    #[default]
    Home,
    Tasks,
    Calendar,
    Marketplace,
    Settings,
    /// The setup checklist, a full page opened from Settings, General (D-OB-12).
    Setup,
    /// Opened from the Workspaces Sidebar, not the rail (A9).
    Automations,
    /// A Bot Teams page. It keeps the Sidebar open, unlike a full page (screens 3, decision 1).
    Team(TeamPage),
}

impl Page {
    /// Full pages cover the Canvas, hide the Sidebar and show Back (A8). Team pages do not.
    pub fn is_full_page(self) -> bool {
        !matches!(self, Page::Home | Page::Team(_))
    }

    pub fn title(self) -> &'static str {
        match self {
            Page::Home => "",
            Page::Tasks => "Tasks",
            Page::Calendar => "Calendar",
            Page::Marketplace => "Marketplace",
            Page::Settings => "Settings",
            Page::Setup => "Setup",
            Page::Automations => "Automations",
            Page::Team(page) => page.title(),
        }
    }
}

/// Where the user is: one mode and one page, plus the pages Back and Forward return to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Navigation {
    pub mode: Mode,
    pub page: Page,
    /// Pages Back returns to, the latest last.
    back_pages: Vec<Page>,
    /// Pages Forward returns to, the latest last.
    forward_pages: Vec<Page>,
}

impl Navigation {
    /// The mode button (P6): from any other page the first press returns to
    /// the mode's home; a press at home switches the mode. Returns true when
    /// the mode changed, so the caller can run the morph.
    pub fn press_mode_switch(&mut self) -> bool {
        if self.page != Page::Home {
            self.page = Page::Home;
            self.back_pages.clear();
            self.forward_pages.clear();
            false
        } else {
            self.mode = self.mode.other();
            true
        }
    }

    /// Opens a page. It becomes the Back target and clears the Forward pages.
    pub fn open(&mut self, page: Page) {
        if page != self.page {
            self.back_pages.push(self.page);
            self.forward_pages.clear();
            self.page = page;
        }
    }

    pub fn can_go_back(&self) -> bool {
        !self.back_pages.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward_pages.is_empty()
    }

    /// Returns to the previous page. Does nothing without history.
    pub fn back(&mut self) {
        if let Some(previous) = self.back_pages.pop() {
            self.forward_pages.push(self.page);
            self.page = previous;
        }
    }

    /// Goes forward again after a Back. Does nothing without forward history.
    pub fn forward(&mut self) {
        if let Some(next) = self.forward_pages.pop() {
            self.back_pages.push(self.page);
            self.page = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_switch_returns_home_first_then_switches() {
        let mut nav = Navigation::default();
        nav.open(Page::Calendar);
        assert!(!nav.press_mode_switch());
        assert_eq!((nav.mode, nav.page), (Mode::Workspaces, Page::Home));
        assert!(nav.press_mode_switch());
        assert_eq!(nav.mode, Mode::BotTeams);
        assert_eq!(nav.mode.switch_tooltip(), "Switch to Workspaces");
    }

    #[test]
    fn forward_replays_what_back_left_and_opening_a_page_clears_it() {
        let mut nav = Navigation::default();
        nav.open(Page::Tasks);
        nav.open(Page::Calendar);
        nav.back();
        assert_eq!(nav.page, Page::Tasks);
        assert!(nav.can_go_forward());
        nav.forward();
        assert_eq!(nav.page, Page::Calendar);
        nav.back();
        nav.open(Page::Marketplace);
        assert!(!nav.can_go_forward());
    }

    #[test]
    fn back_and_forward_do_nothing_without_history() {
        let mut nav = Navigation::default();
        assert!(!nav.can_go_back() && !nav.can_go_forward());
        nav.back();
        nav.forward();
        assert_eq!(nav.page, Page::Home);
    }

    #[test]
    fn back_returns_to_the_previous_page() {
        let mut nav = Navigation::default();
        nav.open(Page::Tasks);
        nav.open(Page::Settings);
        nav.back();
        assert_eq!(nav.page, Page::Tasks);
        nav.back();
        assert_eq!(nav.page, Page::Home);
        assert!(Page::Settings.is_full_page() && !Page::Home.is_full_page());
    }
}
