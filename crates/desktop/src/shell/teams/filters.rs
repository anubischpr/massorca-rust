//! The Board's project filter and the Sidebar's Filters menu (decision 0072). The rows come from the model; this
//! file draws them and applies the row that was chosen.

use super::super::Shell;
use crate::model::teams::{BoardFilterRow, ProjectFilter, TeamId, board_filter_rows};
use gpui::{AppContext, Context, Entity, Pixels, Point, Subscription, WeakEntity, Window};
use massorca_ui::IconName;
use massorca_ui::components::button::{Button, ButtonSize, ButtonVariant};
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::components::menu::{MenuEntry, MenuEvent, MenuItem, MenuView};
use massorca_ui::components::popover::OverlayLayer;

/// The menu on screen, opened by a Board's Filter button or the Sidebar's Filters: its rows and where it opened.
#[derive(Debug)]
pub(in crate::shell) struct OpenFilterMenu {
    pub(in crate::shell) menu: Entity<MenuView>,
    pub(in crate::shell) at: Point<Pixels>,
    rows: Vec<FilterRow>,
    _subscription: Subscription,
}

/// What a row of the open menu does once it is chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FilterRow {
    /// Shows the team's Board under this project filter.
    Board { team: TeamId, filter: ProjectFilter },
    /// Switches whether the Sidebar leaves out the finished projects.
    HideFinished,
}

/// The Filter button of a team's Board. It is drawn as in use while the Board shows fewer than all its cards.
pub(super) fn board_filter_button(team: TeamId, in_use: bool, shell: &WeakEntity<Shell>) -> Button {
    let shell = shell.clone();
    Button::new("board-filter", "Filter")
        .variant(if in_use {
            ButtonVariant::Secondary
        } else {
            ButtonVariant::Ghost
        })
        .leading_icon(IconName::ListFilter)
        .on_click(move |window, cx| {
            let _ = shell.update(cx, |shell, cx| shell.open_board_filter(team, window, cx));
        })
}

/// The Filters button of the Sidebar's PROJECTS heading.
pub(super) fn sidebar_filters_button(shell: &WeakEntity<Shell>) -> IconButton {
    let shell = shell.clone();
    IconButton::new("teams-filters", IconName::ListFilter, "Filters")
        .size(ButtonSize::Compact)
        .on_click(move |window, cx| {
            let _ = shell.update(cx, |shell, cx| shell.open_sidebar_filters(window, cx));
        })
}

impl Shell {
    /// Opens the project filter of `team`'s Board where the pointer is. Nothing opens for a team the model does not hold.
    pub(super) fn open_board_filter(
        &mut self,
        team: TeamId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = self.model.teams();
        let Some(card) = state.team(team) else {
            return;
        };
        let rows: Vec<BoardFilterRow> = board_filter_rows(card, state.board_filter(team));
        let entries: Vec<MenuEntry> = rows.iter().map(board_entry).collect();
        let choices = rows
            .iter()
            .map(|row| FilterRow::Board {
                team,
                filter: row.filter,
            })
            .collect();
        self.open_filter_menu(entries, choices, window, cx);
    }

    /// Opens the Sidebar's Filters menu where the pointer is.
    pub(super) fn open_sidebar_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let hidden = self.model.teams().hides_finished_projects();
        let entries = vec![
            MenuItem::new("Hide finished projects")
                .checked(hidden)
                .into(),
        ];
        self.open_filter_menu(entries, vec![FilterRow::HideFinished], window, cx);
    }

    fn open_filter_menu(
        &mut self,
        entries: Vec<MenuEntry>,
        rows: Vec<FilterRow>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let menu = cx.new(|cx| MenuView::new(entries, window, cx).with_layer(OverlayLayer::Page));
        let subscription = cx.subscribe_in(&menu, window, |shell, _, event: &MenuEvent, _, cx| {
            let open = shell.filter_menu.take();
            if let (MenuEvent::Chosen(choice), Some(open)) = (event, open) {
                shell.filter_menu_chosen(open, choice.row);
            }
            cx.notify();
        });
        self.filter_menu = Some(OpenFilterMenu {
            menu,
            at: window.mouse_position(),
            rows,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Applies the row the menu's choice names, once the menu has closed.
    fn filter_menu_chosen(&mut self, open: OpenFilterMenu, row: usize) {
        match open.rows.get(row).copied() {
            Some(FilterRow::Board { team, filter }) => {
                let _ = self.model.teams_mut().set_board_filter(team, filter);
            }
            Some(FilterRow::HideFinished) => {
                let hidden = self.model.teams().hides_finished_projects();
                self.model.teams_mut().set_hide_finished_projects(!hidden);
            }
            None => {}
        }
    }
}

/// The menu row a Board filter row draws as: a checkable row, checked while it is the filter in use.
fn board_entry(row: &BoardFilterRow) -> MenuEntry {
    MenuItem::new(row.label.clone()).checked(row.checked).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::SampleDataSource;
    use crate::model::teams::{ProjectId, SampleTeamsSource, TeamId, TeamPage};
    use gpui::{Entity, TestAppContext, VisualTestContext};
    use massorca_ui::{Appearance, Theme as UiTheme};

    fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
        cx.update(|cx| {
            UiTheme::set(Appearance::Dark, cx);
            crate::shell::bind_keys(cx);
        });
        cx.add_window_view(|window, cx| {
            Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx)
        })
    }

    #[gpui::test]
    fn the_board_filter_menu_applies_the_project_it_chooses(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            shell.model.open_team_page(TeamPage::Board(TeamId(1)));
            cx.notify();
        });
        shell.update_in(cx, |shell, window, cx| {
            shell.open_board_filter(TeamId(1), window, cx)
        });
        shell.update(cx, |shell, cx| {
            let open = shell.filter_menu.take().expect("the filter menu is open");
            shell.filter_menu_chosen(open, 1);
            cx.notify();
        });
        shell.read_with(cx, |shell, _| {
            assert_eq!(
                shell.model().teams().board_filter(TeamId(1)),
                ProjectFilter::Project(ProjectId(1))
            );
        });
    }

    #[gpui::test]
    fn the_sidebar_filters_menu_hides_the_finished_projects(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        shell.update_in(cx, |shell, window, cx| {
            shell.open_sidebar_filters(window, cx)
        });
        shell.update(cx, |shell, cx| {
            let open = shell.filter_menu.take().expect("the Filters menu is open");
            shell.filter_menu_chosen(open, 0);
            cx.notify();
        });
        shell.read_with(cx, |shell, _| {
            assert!(shell.model().teams().hides_finished_projects());
        });
    }
}
