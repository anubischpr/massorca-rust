//! How the window opens. `MASSORCA_DESKTOP_BACKGROUND=1` opens it hidden and unfocused, for
//! automated checks on a desktop someone is using. GPUI on Windows moves bounds whose centre is
//! on no monitor back onto the primary display, so an off-screen origin cannot be asked for at
//! open time: the driver shows the hidden window where it wants, without activating it.
//! `MASSORCA_DESKTOP_DEBUG_FOCUS=search` or `=composer` (read only with the background launch) puts
//! the focus in the Search field or the start page composer and logs what it settles on, so a check
//! can type into a field it has not activated.

use std::str::FromStr;

use crate::service::truthy;

const BACKGROUND_ENV: &str = "MASSORCA_DESKTOP_BACKGROUND";
const DEBUG_FOCUS_ENV: &str = "MASSORCA_DESKTOP_DEBUG_FOCUS";

/// A page or dialog a launch opens with `--page <name>`. The names are the command line's and
/// the documented ones (`docs/cli.md`), and `ALL` is the one list the parser and the help use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugPage {
    Workspace,
    SettingsGeneral,
    SettingsHosts,
    SettingsVault,
    Pairing,
    Board,
    AddHost,
    AddSecret,
    ReplaceSecret,
    RemoveSecret,
    NewTeam,
    AddBot,
    Session,
}

impl DebugPage {
    /// Every page, in the order the help lists them.
    pub const ALL: [DebugPage; 13] = [
        DebugPage::Workspace,
        DebugPage::SettingsGeneral,
        DebugPage::SettingsHosts,
        DebugPage::SettingsVault,
        DebugPage::Pairing,
        DebugPage::Board,
        DebugPage::AddHost,
        DebugPage::AddSecret,
        DebugPage::ReplaceSecret,
        DebugPage::RemoveSecret,
        DebugPage::NewTeam,
        DebugPage::AddBot,
        DebugPage::Session,
    ];

    /// The name `--page` takes for this page.
    pub fn name(self) -> &'static str {
        match self {
            DebugPage::Workspace => "workspace",
            DebugPage::SettingsGeneral => "settings-general",
            DebugPage::SettingsHosts => "settings-hosts",
            DebugPage::SettingsVault => "settings-vault",
            DebugPage::Pairing => "pairing",
            DebugPage::Board => "board",
            DebugPage::AddHost => "add-host",
            DebugPage::AddSecret => "add-secret",
            DebugPage::ReplaceSecret => "replace-secret",
            DebugPage::RemoveSecret => "remove-secret",
            DebugPage::NewTeam => "new-team",
            DebugPage::AddBot => "add-bot",
            DebugPage::Session => "session",
        }
    }

    /// Every page name, comma separated, in `ALL` order.
    pub fn names() -> String {
        Self::ALL
            .iter()
            .map(|page| page.name())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl FromStr for DebugPage {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|page| page.name() == value)
            .ok_or_else(|| format!("unknown page \"{value}\"; the pages are: {}", Self::names()))
    }
}

/// A field the debug launch focuses and logs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugFocus {
    /// The sidebar Search field.
    Search,
    /// The composer of the first workspace's start page.
    Composer,
}

impl DebugFocus {
    fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "search" => Some(Self::Search),
            "composer" => Some(Self::Composer),
            _ => None,
        }
    }
}

/// How the main window opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowLaunch {
    /// Open hidden, never take focus and never activate the app.
    pub background: bool,
    /// Debug only: focus this field once the first frame is drawn, and log its values.
    pub debug_focus: Option<DebugFocus>,
}

impl WindowLaunch {
    /// Reads `MASSORCA_DESKTOP_BACKGROUND` with the CLI's falsey rule, and the debug focus only
    /// with it, so a normal launch never changes where the focus starts.
    pub fn from_env(env: impl Fn(&str) -> Option<String>) -> Self {
        let background = env(BACKGROUND_ENV).is_some_and(|value| truthy(&value));
        let debug_focus = background
            .then(|| env(DEBUG_FOCUS_ENV))
            .flatten()
            .and_then(|value| DebugFocus::parse(&value));
        Self {
            background,
            debug_focus,
        }
    }

    /// Whether the window takes focus when it opens.
    pub fn focus(self) -> bool {
        !self.background
    }

    /// Whether the window is shown when it opens.
    pub fn show(self) -> bool {
        !self.background
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch(value: Option<&str>) -> WindowLaunch {
        WindowLaunch::from_env(|key| {
            (key == BACKGROUND_ENV)
                .then_some(value)
                .flatten()
                .map(str::to_owned)
        })
    }

    fn launch_with(background: &str, debug_focus: &str) -> WindowLaunch {
        WindowLaunch::from_env(|key| match key {
            BACKGROUND_ENV => Some(background.to_owned()),
            DEBUG_FOCUS_ENV => Some(debug_focus.to_owned()),
            _ => None,
        })
    }

    #[test]
    fn without_the_variable_the_window_shows_and_takes_focus() {
        let normal = launch(None);
        assert_eq!(normal, WindowLaunch::default());
        assert!(normal.show() && normal.focus());
    }

    #[test]
    fn the_background_variable_opens_the_window_hidden_and_unfocused() {
        let background = launch(Some("1"));
        assert!(background.background);
        assert!(!background.show() && !background.focus());
    }

    #[test]
    fn falsey_spellings_keep_the_normal_launch() {
        for value in ["", "0", "false", "OFF", " no "] {
            assert!(!launch(Some(value)).background, "{value:?}");
        }
    }

    #[test]
    fn the_debug_focus_needs_the_background_launch() {
        assert_eq!(
            launch_with("1", "search").debug_focus,
            Some(DebugFocus::Search)
        );
        assert_eq!(launch_with("0", "search").debug_focus, None);
        assert_eq!(
            launch_with("1", "composer").debug_focus,
            Some(DebugFocus::Composer)
        );
        assert_eq!(launch_with("0", "composer").debug_focus, None);
        assert_eq!(launch(None).debug_focus, None);
    }

    #[test]
    fn unknown_debug_focus_values_are_ignored() {
        for value in ["", "chat", "Search!"] {
            assert_eq!(launch_with("1", value).debug_focus, None, "{value:?}");
        }
    }

    #[test]
    fn the_page_names_are_the_documented_ones() {
        assert_eq!(
            DebugPage::names(),
            "workspace, settings-general, settings-hosts, settings-vault, pairing, board, \
             add-host, add-secret, replace-secret, remove-secret, new-team, add-bot, session"
        );
    }

    #[test]
    fn the_help_lists_exactly_the_pages_of_the_enum() {
        let names: Vec<&str> = DebugPage::ALL.iter().map(|page| page.name()).collect();
        assert_eq!(DebugPage::names(), names.join(", "));
        assert_eq!(
            names
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            DebugPage::ALL.len(),
            "every name is unique"
        );
    }

    #[test]
    fn the_session_page_is_named_session() {
        assert_eq!("session".parse::<DebugPage>(), Ok(DebugPage::Session));
    }

    #[test]
    fn every_page_name_parses_back_to_its_page() {
        for page in DebugPage::ALL {
            assert_eq!(page.name().parse::<DebugPage>(), Ok(page));
        }
    }

    #[test]
    fn an_unknown_page_name_is_refused_with_the_valid_names() {
        for value in ["", "Board", "board ", "settings", "nope"] {
            let error = value.parse::<DebugPage>().expect_err(value);
            assert_eq!(
                error,
                format!(
                    "unknown page \"{value}\"; the pages are: {}",
                    DebugPage::names()
                )
            );
        }
    }
}
