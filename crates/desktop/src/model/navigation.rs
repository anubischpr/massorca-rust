//! Modes and pages: the Activity rail and the mode switch (D-SH-09, D-SH-10).

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
}

impl Page {
    /// Full pages cover the Canvas, hide the Sidebar and show Back (A8).
    pub fn is_full_page(self) -> bool {
        self != Page::Home
    }

    pub fn title(self) -> &'static str {
        match self {
            Page::Home => "",
            Page::Tasks => "Tasks",
            Page::Calendar => "Calendar",
            Page::Marketplace => "Marketplace",
            Page::Settings => "Settings",
        }
    }
}

/// Where the user is: one mode and one page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Navigation {
    pub mode: Mode,
    pub page: Page,
    /// The page Back returns to.
    previous: Option<Page>,
}

impl Navigation {
    /// The mode button (P6): from any other page the first press returns to
    /// the mode's home; a press at home switches the mode. Returns true when
    /// the mode changed, so the caller can run the morph.
    pub fn press_mode_switch(&mut self) -> bool {
        if self.page != Page::Home {
            self.page = Page::Home;
            self.previous = None;
            false
        } else {
            self.mode = self.mode.other();
            true
        }
    }

    pub fn open(&mut self, page: Page) {
        if page != self.page {
            self.previous = Some(self.page);
            self.page = page;
        }
    }

    pub fn back(&mut self) {
        self.page = self.previous.take().unwrap_or(Page::Home);
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
