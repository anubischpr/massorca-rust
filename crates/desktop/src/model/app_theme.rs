//! The app theme that `appearance.theme` asks for: its choices, their words and labels, and what
//! "system" shows. Free of GPUI, so the rules test without a window.

use massorca_protocol::methods::config::ConfigValue;
use massorca_ui::Appearance;

/// The service setting the rail toggle and the Settings theme row write.
pub const APPEARANCE_THEME_KEY: &str = "appearance.theme";

/// The choices of `appearance.theme`, in the order the Settings row shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeChoice {
    /// Follows the operating system's appearance.
    System,
    Dark,
    Light,
}

impl ThemeChoice {
    pub const ALL: [Self; 3] = [Self::System, Self::Dark, Self::Light];

    /// The choice a stored word names; a word the service does not declare names none.
    pub fn from_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.word() == word)
    }

    /// The choice the service stores, or `None` while it is not known.
    pub fn from_setting(setting: Option<&ConfigValue>) -> Option<Self> {
        setting?.value.as_str().and_then(Self::from_word)
    }

    /// The word the service stores for this choice.
    pub fn word(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    /// The label the Settings row shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }

    /// The segment this choice is, in `ALL`.
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|choice| *choice == self)
            .unwrap_or(0)
    }

    /// The appearance this choice shows, given the operating system's appearance.
    pub fn resolve(self, system: Appearance) -> Appearance {
        match self {
            Self::System => system,
            Self::Dark => Appearance::Dark,
            Self::Light => Appearance::Light,
        }
    }

    /// The fixed choice for the other appearance, which the rail toggle writes.
    pub fn other_than(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Dark => Self::Light,
            Appearance::Light => Self::Dark,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(word: &str) -> ConfigValue {
        ConfigValue {
            value: word.into(),
            ..Default::default()
        }
    }

    #[test]
    fn the_three_words_the_service_declares_are_the_only_choices() {
        let words: Vec<&str> = ThemeChoice::ALL
            .iter()
            .map(|choice| choice.word())
            .collect();
        assert_eq!(words, ["system", "dark", "light"]);
        for choice in ThemeChoice::ALL {
            assert_eq!(ThemeChoice::from_word(choice.word()), Some(choice));
        }
        assert_eq!(ThemeChoice::from_word("Dark"), None);
        assert_eq!(ThemeChoice::from_word("auto"), None);
    }

    #[test]
    fn the_stored_setting_names_a_choice_only_when_it_is_known() {
        assert_eq!(ThemeChoice::from_setting(None), None);
        assert_eq!(
            ThemeChoice::from_setting(Some(&stored("light"))),
            Some(ThemeChoice::Light)
        );
        assert_eq!(ThemeChoice::from_setting(Some(&stored("sepia"))), None);
    }

    #[test]
    fn the_segments_follow_the_order_of_all() {
        let labels: Vec<&str> = ThemeChoice::ALL
            .iter()
            .map(|choice| choice.label())
            .collect();
        assert_eq!(labels, ["System", "Dark", "Light"]);
        assert_eq!(ThemeChoice::System.index(), 0);
        assert_eq!(ThemeChoice::Light.index(), 2);
    }

    #[test]
    fn system_follows_the_operating_system_and_the_others_do_not() {
        assert_eq!(
            ThemeChoice::System.resolve(Appearance::Light),
            Appearance::Light
        );
        assert_eq!(
            ThemeChoice::System.resolve(Appearance::Dark),
            Appearance::Dark
        );
        assert_eq!(
            ThemeChoice::Dark.resolve(Appearance::Light),
            Appearance::Dark
        );
        assert_eq!(
            ThemeChoice::Light.resolve(Appearance::Dark),
            Appearance::Light
        );
    }

    #[test]
    fn the_rail_toggle_asks_for_the_other_appearance() {
        assert_eq!(
            ThemeChoice::other_than(Appearance::Dark),
            ThemeChoice::Light
        );
        assert_eq!(
            ThemeChoice::other_than(Appearance::Light),
            ThemeChoice::Dark
        );
    }
}
