//! The palette's Recent rows: the commands and workspaces the palette ran last, most recent first,
//! kept in memory per window (CommandPalette spec). Sessions are not kept: the Sessions group
//! already finds them.

use super::palette::{PaletteAction, PaletteGroup, PaletteItem};

/// How many recent rows the palette keeps and shows (CommandPalette spec: five, proposed).
pub const RECENT_LIMIT: usize = 5;

/// The actions run from the palette, most recent first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecentActions {
    actions: Vec<PaletteAction>,
}

impl RecentActions {
    /// Puts a run action at the front, once, and drops the oldest past the limit.
    pub fn record(&mut self, action: &PaletteAction) {
        if matches!(action, PaletteAction::OpenSession(_)) {
            return;
        }
        self.actions.retain(|kept| kept != action);
        self.actions.insert(0, action.clone());
        self.actions.truncate(RECENT_LIMIT);
    }

    /// The Recent rows for the palette's items, in the order they were run. An action that is no
    /// longer listed, such as a removed workspace, leaves the list.
    pub fn rows(&self, items: &[PaletteItem]) -> Vec<PaletteItem> {
        self.actions
            .iter()
            .filter_map(|action| items.iter().find(|item| &item.action == action))
            .map(|item| PaletteItem {
                group: PaletteGroup::Recent,
                ..item.clone()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{DataSource, SampleDataSource};
    use crate::model::navigation::Page;
    use crate::model::palette::{Command, palette_items};

    fn toggle_theme() -> PaletteAction {
        PaletteAction::Command(Command::ToggleTheme)
    }

    #[test]
    fn the_last_run_comes_first_and_a_repeat_moves_to_the_front_once() {
        let mut recent = RecentActions::default();
        recent.record(&PaletteAction::OpenWorkspace("w1".into()));
        recent.record(&PaletteAction::OpenWorkspace("w2".into()));
        recent.record(&PaletteAction::OpenWorkspace("w1".into()));
        assert_eq!(
            recent.actions,
            vec![
                PaletteAction::OpenWorkspace("w1".into()),
                PaletteAction::OpenWorkspace("w2".into()),
            ]
        );
    }

    #[test]
    fn only_the_last_five_runs_are_kept() {
        let mut recent = RecentActions::default();
        for n in 0..(RECENT_LIMIT + 2) {
            recent.record(&PaletteAction::OpenWorkspace(format!("w{n}")));
        }
        assert_eq!(recent.actions.len(), RECENT_LIMIT);
        assert_eq!(
            recent.actions.first(),
            Some(&PaletteAction::OpenWorkspace(format!(
                "w{}",
                RECENT_LIMIT + 1
            )))
        );
        assert_eq!(
            recent.actions.last(),
            Some(&PaletteAction::OpenWorkspace("w2".into()))
        );
    }

    #[test]
    fn sessions_are_not_recorded() {
        let mut recent = RecentActions::default();
        recent.record(&PaletteAction::OpenSession("s1".into()));
        assert!(recent.actions.is_empty());
    }

    #[test]
    fn rows_are_copies_in_run_order_and_skip_what_is_gone() {
        let items = palette_items(&SampleDataSource.snapshot());
        let mut recent = RecentActions::default();
        recent.record(&toggle_theme());
        recent.record(&PaletteAction::Command(Command::Open(Page::Tasks)));
        recent.record(&PaletteAction::OpenWorkspace("removed".into()));
        let rows = recent.rows(&items);
        let titles: Vec<&str> = rows.iter().map(|row| row.title.as_str()).collect();
        assert_eq!(titles, ["Open tasks", "Toggle theme"]);
        assert!(rows.iter().all(|row| row.group == PaletteGroup::Recent));
    }
}
