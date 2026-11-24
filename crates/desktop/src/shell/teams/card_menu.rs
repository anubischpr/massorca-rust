//! The menu of a team card, opened by its More button: Edit team, the one-step access change and Delete team
//! (screen 4.5). Its rows come from the model; this file draws them and runs the row that was chosen.

use super::super::Shell;
use super::dialogs::TeamConfirm;
use crate::model::teams::{CardMenuRow, TeamId, card_menu_rows};
use gpui::{AppContext, Context, Entity, Pixels, Point, Subscription, Window};
use massorca_ui::IconName;
use massorca_ui::components::menu::{MenuEntry, MenuEvent, MenuItem, MenuView};
use massorca_ui::components::popover::OverlayLayer;

/// The team card menu on screen: its rows, the team they act on, and where it opened.
#[derive(Debug)]
pub(in crate::shell) struct OpenTeamMenu {
    pub(in crate::shell) menu: Entity<MenuView>,
    pub(in crate::shell) at: Point<Pixels>,
    team: TeamId,
    rows: Vec<CardMenuRow>,
    _subscription: Subscription,
}

impl Shell {
    /// Opens the menu of `team` where the pointer is, as the workspace menus open. Nothing opens for a team the
    /// model does not hold.
    pub(super) fn open_team_menu(
        &mut self,
        team: TeamId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(card) = self.model.teams().team(team) else {
            return;
        };
        let rows = card_menu_rows(card);
        let entries: Vec<MenuEntry> = rows.iter().map(|row| menu_entry(*row)).collect();
        let menu = cx.new(|cx| MenuView::new(entries, window, cx).with_layer(OverlayLayer::Page));
        let subscription =
            cx.subscribe_in(&menu, window, |shell, _, event: &MenuEvent, window, cx| {
                let open = shell.team_menu.take();
                if let (MenuEvent::Chosen(choice), Some(open)) = (event, open) {
                    shell.team_menu_chosen(open, choice.row, window, cx);
                }
                cx.notify();
            });
        self.team_menu = Some(OpenTeamMenu {
            menu,
            at: window.mouse_position(),
            team,
            rows,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Runs the row the menu's choice names, once the menu has closed.
    fn team_menu_chosen(
        &mut self,
        open: OpenTeamMenu,
        row: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match open.rows.get(row).copied() {
            Some(CardMenuRow::Edit) => self.open_team_edit(open.team, window, cx),
            Some(CardMenuRow::Access { access, .. }) => self.set_team_access(open.team, access, cx),
            Some(CardMenuRow::Delete) => self.open_team_delete(open.team, cx),
            Some(CardMenuRow::Separator) | None => {}
        }
    }

    /// "Delete team…" on a card: a confirm that names the team and says what goes with it.
    fn open_team_delete(&mut self, team: TeamId, cx: &mut Context<Self>) {
        let Some(name) = self.model.teams().team(team).map(|team| team.name.clone()) else {
            return;
        };
        self.open_team_confirm(TeamConfirm::RemoveTeam { team, name }, cx);
    }
}

/// The menu row a card row draws as.
fn menu_entry(row: CardMenuRow) -> MenuEntry {
    match row {
        CardMenuRow::Edit => MenuItem::new("Edit team…").icon(IconName::Pencil).into(),
        CardMenuRow::Access { label, .. } => MenuItem::new(label).icon(IconName::Shield).into(),
        CardMenuRow::Separator => MenuEntry::Separator,
        CardMenuRow::Delete => MenuItem::new("Delete team…")
            .icon(IconName::Trash2)
            .destructive()
            .into(),
    }
}
