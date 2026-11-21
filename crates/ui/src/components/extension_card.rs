//! Extension card (docs/design/components/ExtensionCard.md): one extension, skill, tool, agent or
//! service in the Marketplace, with the one action that fits its state. Desktop only.

use gpui::SharedString;

/// Chips drawn before the rest fold into "+N": two rows of two at the minimum width.
const VISIBLE_CHIPS: usize = 4;

/// Columns of the extension grid for a page width: one under 560 px, two from 560, three from 880.
pub fn grid_columns(width: f32) -> usize {
    if width >= 880.0 {
        3
    } else if width >= 560.0 {
        2
    } else {
        1
    }
}

/// The chips drawn and the number folded into "+N".
pub fn visible_chips(total: usize) -> (usize, usize) {
    let shown = total.min(VISIBLE_CHIPS);
    (shown, total - shown)
}

/// The state line of a host that cannot be reached: the last known state and its age.
pub fn last_known_line(enabled: bool, age: &str) -> String {
    let state = if enabled { "enabled" } else { "disabled" };
    format!("Last known: {state}, {age}")
}

/// What the card's footer shows: the one action that fits the extension's state.
#[derive(Clone, Debug, PartialEq)]
pub enum ExtensionState {
    /// In the catalog and not installed: the install action.
    Catalog,
    /// Installed; `update` is the version on offer, which replaces the switch.
    Installed {
        enabled: bool,
        update: Option<SharedString>,
    },
    /// Installing, from 0 to 100 percent.
    Installing { percent: u8 },
    /// Installed but failed to start; the reason is one line.
    Failed { reason: SharedString },
    /// Needs a newer MassOrca, given as its version.
    Incompatible { requires: SharedString },
}

/// What a press on the card asks for. The caller acts on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionAction {
    Open,
    Install,
    Update,
    Cancel,
    Toggle(bool),
    Settings,
    More,
    ViewLog,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_has_one_two_or_three_columns_by_width() {
        assert_eq!(grid_columns(559.0), 1);
        assert_eq!(grid_columns(560.0), 2);
        assert_eq!(grid_columns(879.0), 2);
        assert_eq!(grid_columns(880.0), 3);
    }

    #[test]
    fn chips_fold_the_rest_into_a_count() {
        assert_eq!(visible_chips(2), (2, 0));
        assert_eq!(visible_chips(7), (4, 3));
    }

    #[test]
    fn last_known_line_names_the_state_and_its_age() {
        assert_eq!(
            last_known_line(true, "5 min ago"),
            "Last known: enabled, 5 min ago"
        );
        assert_eq!(
            last_known_line(false, "1 h ago"),
            "Last known: disabled, 1 h ago"
        );
    }
}
