//! How the window opens. `MASSORCA_DESKTOP_BACKGROUND=1` opens it hidden and unfocused, for
//! automated checks on a desktop someone is using. GPUI on Windows moves bounds whose centre is
//! on no monitor back onto the primary display, so an off-screen origin cannot be asked for at
//! open time: the driver shows the hidden window where it wants, without activating it.
//! `MASSORCA_DESKTOP_DEBUG_FOCUS=search` (read only with the background launch) puts the focus in
//! the Search field and logs its query, so a check can type into a field it has not activated.

use crate::service::truthy;

const BACKGROUND_ENV: &str = "MASSORCA_DESKTOP_BACKGROUND";
const DEBUG_FOCUS_ENV: &str = "MASSORCA_DESKTOP_DEBUG_FOCUS";

/// A field the debug launch focuses and logs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugFocus {
    /// The sidebar Search field.
    Search,
}

impl DebugFocus {
    fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "search" => Some(Self::Search),
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
        assert_eq!(launch(None).debug_focus, None);
    }

    #[test]
    fn unknown_debug_focus_values_are_ignored() {
        for value in ["", "composer", "Search!"] {
            assert_eq!(launch_with("1", value).debug_focus, None, "{value:?}");
        }
    }
}
