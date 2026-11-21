//! How the window opens. `MASSORCA_DESKTOP_BACKGROUND=1` opens it hidden and unfocused, for
//! automated checks on a desktop someone is using. GPUI on Windows moves bounds whose centre is
//! on no monitor back onto the primary display, so an off-screen origin cannot be asked for at
//! open time: the driver shows the hidden window where it wants, without activating it.

use crate::service::truthy;

const BACKGROUND_ENV: &str = "MASSORCA_DESKTOP_BACKGROUND";

/// How the main window opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowLaunch {
    /// Open hidden, never take focus and never activate the app.
    pub background: bool,
}

impl WindowLaunch {
    /// Reads `MASSORCA_DESKTOP_BACKGROUND` with the CLI's falsey rule.
    pub fn from_env(env: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            background: env(BACKGROUND_ENV).is_some_and(|value| truthy(&value)),
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
}
