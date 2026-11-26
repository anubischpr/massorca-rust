//! The menu of a routine card, opened by its More button: History, Edit, Pause or Resume, and Delete, which asks first.

use super::super::Shell;
use crate::model::teams::{RoutineId, TeamId, TeamPage};
use gpui::{AppContext, Context, Entity, Pixels, Point, Subscription, Window};
use massorca_ui::IconName;
use massorca_ui::components::menu::{MenuEntry, MenuEvent, MenuItem, MenuView};
use massorca_ui::components::popover::OverlayLayer;

/// The rows of a routine's menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RoutineMenuRow {
    History,
    Edit,
    Pause,
    Resume,
    Delete,
}

/// The rows a routine's menu shows: the pause row names what the routine's state lets it do.
fn routine_menu_rows(enabled: bool) -> Vec<RoutineMenuRow> {
    let pause = if enabled {
        RoutineMenuRow::Pause
    } else {
        RoutineMenuRow::Resume
    };
    vec![
        RoutineMenuRow::History,
        RoutineMenuRow::Edit,
        pause,
        RoutineMenuRow::Delete,
    ]
}

/// The routine menu on screen: the routine it acts on, the rows it shows and where it opened.
#[derive(Debug)]
pub(in crate::shell) struct OpenRoutineMenu {
    pub(in crate::shell) menu: Entity<MenuView>,
    pub(in crate::shell) at: Point<Pixels>,
    team: TeamId,
    routine: RoutineId,
    rows: Vec<RoutineMenuRow>,
    _subscription: Subscription,
}

impl Shell {
    /// Opens the menu of `routine` where the pointer is, as the memory entry menu opens.
    pub(super) fn open_routine_menu(
        &mut self,
        team: TeamId,
        routine: RoutineId,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rows = routine_menu_rows(enabled);
        let entries: Vec<MenuEntry> = rows.iter().map(|row| menu_entry(*row)).collect();
        let menu = cx.new(|cx| MenuView::new(entries, window, cx).with_layer(OverlayLayer::Page));
        let subscription = cx.subscribe_in(&menu, window, |shell, _, event: &MenuEvent, _, cx| {
            let open = shell.routine_menu.take();
            if let (MenuEvent::Chosen(choice), Some(open)) = (event, open) {
                shell.routine_menu_chosen(open, choice.row, cx);
            }
            cx.notify();
        });
        self.routine_menu = Some(OpenRoutineMenu {
            menu,
            at: window.mouse_position(),
            team,
            routine,
            rows,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Runs the row the menu's choice names, once the menu has closed.
    fn routine_menu_chosen(&mut self, open: OpenRoutineMenu, row: usize, cx: &mut Context<Self>) {
        let (team, routine) = (open.team, open.routine);
        match open.rows.get(row) {
            Some(RoutineMenuRow::History) => self.open_routine_history(team, routine, cx),
            Some(RoutineMenuRow::Edit) => {
                self.open_routine_form(TeamPage::EditRoutine(team, routine), cx);
            }
            Some(RoutineMenuRow::Pause) => self.set_routine_enabled(team, routine, false, cx),
            Some(RoutineMenuRow::Resume) => self.set_routine_enabled(team, routine, true, cx),
            Some(RoutineMenuRow::Delete) => self.open_routine_confirm(team, routine, cx),
            None => {}
        }
    }
}

/// The menu row a routine row draws as; Delete is marked destructive.
fn menu_entry(row: RoutineMenuRow) -> MenuEntry {
    match row {
        RoutineMenuRow::History => MenuItem::new("History").icon(IconName::Clock).into(),
        RoutineMenuRow::Edit => MenuItem::new("Edit").icon(IconName::Pencil).into(),
        RoutineMenuRow::Pause => MenuItem::new("Pause").into(),
        RoutineMenuRow::Resume => MenuItem::new("Resume").into(),
        RoutineMenuRow::Delete => MenuItem::new("Delete")
            .icon(IconName::Trash2)
            .destructive()
            .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_running_routine_offers_pause_and_a_paused_one_resume() {
        assert_eq!(
            routine_menu_rows(true),
            [
                RoutineMenuRow::History,
                RoutineMenuRow::Edit,
                RoutineMenuRow::Pause,
                RoutineMenuRow::Delete
            ]
        );
        assert_eq!(routine_menu_rows(false)[2], RoutineMenuRow::Resume);
    }
}
