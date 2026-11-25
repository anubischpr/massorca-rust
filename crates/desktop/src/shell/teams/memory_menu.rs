//! The menu of a memory entry, opened by its More button: Delete, which asks first (screen 4.8). The row runs the
//! confirm the entry's trash button used to open; this file draws the menu and runs the row that was chosen.

use super::super::Shell;
use super::dialogs::TeamConfirm;
use crate::model::teams::{MemoryId, TeamId};
use gpui::{AppContext, Context, Entity, Pixels, Point, Subscription, Window};
use massorca_ui::IconName;
use massorca_ui::components::menu::{MenuEntry, MenuEvent, MenuItem, MenuView};
use massorca_ui::components::popover::OverlayLayer;

/// The rows of a memory entry's menu, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemoryMenuRow {
    Delete,
}

const MEMORY_MENU_ROWS: [MemoryMenuRow; 1] = [MemoryMenuRow::Delete];

/// The memory menu on screen: the entry it acts on and where it opened.
#[derive(Debug)]
pub(in crate::shell) struct OpenMemoryMenu {
    pub(in crate::shell) menu: Entity<MenuView>,
    pub(in crate::shell) at: Point<Pixels>,
    team: TeamId,
    memory: MemoryId,
    _subscription: Subscription,
}

impl Shell {
    /// Opens the menu of `memory` where the pointer is, as the team card menu opens.
    pub(super) fn open_memory_menu(
        &mut self,
        team: TeamId,
        memory: MemoryId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entries: Vec<MenuEntry> = MEMORY_MENU_ROWS
            .iter()
            .map(|row| menu_entry(*row))
            .collect();
        let menu = cx.new(|cx| MenuView::new(entries, window, cx).with_layer(OverlayLayer::Page));
        let subscription = cx.subscribe_in(&menu, window, |shell, _, event: &MenuEvent, _, cx| {
            let open = shell.memory_menu.take();
            if let (MenuEvent::Chosen(choice), Some(open)) = (event, open) {
                shell.memory_menu_chosen(open, choice.row, cx);
            }
            cx.notify();
        });
        self.memory_menu = Some(OpenMemoryMenu {
            menu,
            at: window.mouse_position(),
            team,
            memory,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Runs the row the menu's choice names, once the menu has closed.
    fn memory_menu_chosen(&mut self, open: OpenMemoryMenu, row: usize, cx: &mut Context<Self>) {
        if let Some(MemoryMenuRow::Delete) = MEMORY_MENU_ROWS.get(row) {
            let confirm = TeamConfirm::DeleteMemory {
                team: open.team,
                memory: open.memory,
            };
            self.open_team_confirm(confirm, cx);
        }
    }
}

/// The menu row a memory row draws as: Delete, marked destructive.
fn menu_entry(row: MemoryMenuRow) -> MenuEntry {
    match row {
        MemoryMenuRow::Delete => MenuItem::new("Delete")
            .icon(IconName::Trash2)
            .destructive()
            .into(),
    }
}
