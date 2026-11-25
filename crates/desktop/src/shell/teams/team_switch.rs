//! The team pill's switch (A59, screen 4.3): a menu of the teams that opens the same page of the team chosen.

use super::super::Shell;
use crate::model::teams::{TeamPage, TeamSwitchRow, team_switch_rows};
use gpui::{AppContext, Context, Entity, Pixels, Point, Subscription, Window};
use massorca_ui::components::menu::{MenuEntry, MenuEvent, MenuItem, MenuView};
use massorca_ui::components::popover::OverlayLayer;

/// The switch menu on screen: its rows, the page it switches and where it opened.
#[derive(Debug)]
pub(in crate::shell) struct OpenTeamSwitch {
    pub(in crate::shell) menu: Entity<MenuView>,
    pub(in crate::shell) at: Point<Pixels>,
    page: TeamPage,
    rows: Vec<TeamSwitchRow>,
    _subscription: Subscription,
}

impl Shell {
    /// Opens the switch of `page`'s team where the pointer is, as the team card menu opens.
    pub(super) fn open_team_switch(
        &mut self,
        page: TeamPage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rows = team_switch_rows(self.model.teams().teams(), page.team());
        let entries: Vec<MenuEntry> = rows
            .iter()
            .map(|row| MenuItem::new(row.name.clone()).checked(row.current).into())
            .collect();
        let menu = cx.new(|cx| MenuView::new(entries, window, cx).with_layer(OverlayLayer::Page));
        let subscription = cx.subscribe_in(&menu, window, |shell, _, event: &MenuEvent, _, cx| {
            let open = shell.team_switch.take();
            if let (MenuEvent::Chosen(choice), Some(open)) = (event, open)
                && let Some(row) = open.rows.get(choice.row)
                && !row.current
            {
                shell.model.open_team_page(open.page.for_team(row.team));
            }
            cx.notify();
        });
        self.team_switch = Some(OpenTeamSwitch {
            menu,
            at: window.mouse_position(),
            page,
            rows,
            _subscription: subscription,
        });
        cx.notify();
    }
}
