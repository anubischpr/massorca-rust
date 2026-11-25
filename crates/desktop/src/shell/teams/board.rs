//! The Board (D-WS-30, screens 4.3): four columns of cards. Space picks a card up, the arrow keys move
//! it between columns and places, Space drops it and Escape puts it back. The Board changes only on
//! the drop, and a drop asks the service for the call its move makes (`board_move`).

use super::super::Shell;
use super::super::sidebar::row_focus;
use super::access::access_control;
use super::filters::board_filter_button;
use super::team_pill;
use crate::model::shell::HostReach;
use crate::model::shell_state::ShellModel;
use crate::model::teams::{
    BoardMove, BoardOwner, CardView, Column, PickStep, ProjectFilter, TaskId, TeamError, TeamPage,
    board_move,
};
use crate::service::{Refusal, ServiceHandle};
use gpui::{
    AnyElement, App, Context, FocusHandle, ParentElement, Styled, WeakEntity, Window, div,
    prelude::*, px,
};
use massorca_protocol::methods::tasks::Task;
use massorca_protocol::methods::teams::TeamCharacter;
use massorca_ui::components::agent_status::AgentState as GlyphState;
use massorca_ui::components::board_column::{BoardCard, BoardColumn};
use massorca_ui::components::scroll_area::{ScrollArea, ScrollAxis};
use massorca_ui::tokens::{SPACE_3, SPACE_4, SPACE_5, TYPE_BODY_SM, TYPE_CAPTION, TYPE_TITLE_SM};
use massorca_ui::{ActiveTheme, TextStyled, Theme};
use std::future::Future;

/// The keys a picked card reads: Space, the arrows and Escape.
const PICK_KEYS: [&str; 6] = ["space", "left", "right", "up", "down", "escape"];

/// The height of the Board's header row, the rule under it included (bt-board `.head`, 56 px).
const HEADER_HEIGHT: f32 = 56.0;

/// The Board page of a team: its header, the team pill and the four columns (4.3).
pub(in crate::shell) fn board_page(
    model: &ShellModel,
    shell: &WeakEntity<Shell>,
    owner: BoardOwner,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = *cx.theme();
    let state = model.teams();
    let (team, page) = match owner {
        BoardOwner::Team(team) => (team, TeamPage::Board(team)),
        BoardOwner::Project(team, project) => (team, TeamPage::Project(team, project)),
    };
    let team_data = state.team(team);
    let team_name = team_data.map_or("", |team| team.name.as_str());
    let character = team_data.map_or(TeamCharacter::Unknown, |team| team.character);
    // One row holds the title, the team pill and the note, with the Filter button at the end and a
    // rule under them (bt-board `.head`).
    let header = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(SPACE_3))
        .h(px(HEADER_HEIGHT))
        .px(px(SPACE_5))
        .border_b_1()
        .border_color(theme.palette.border.subtle)
        .debug_selector(|| "board-header".to_owned())
        .child(
            div()
                .text_style(TYPE_TITLE_SM)
                .text_color(theme.palette.text.primary)
                .child("Board"),
        )
        .child(team_pill(page, team_name, character, shell, window, cx))
        .children(team_data.map(|team| {
            access_control(
                team,
                state.access_notice(team.id),
                shell,
                state.is_connected(),
                &theme,
            )
        }))
        .child(
            div()
                .text_style(TYPE_CAPTION)
                .text_color(theme.palette.text.tertiary)
                .child("Tasks come from the Conference room"),
        )
        .child(div().flex_1())
        .children(match owner {
            BoardOwner::Team(team) => Some(board_filter_button(
                team,
                state.board_filter(team) != ProjectFilter::All,
                shell,
            )),
            BoardOwner::Project(..) => None,
        });
    // The columns start 16 px under the header and run to the window edge, where they scroll.
    div()
        .flex()
        .flex_col()
        .size_full()
        .child(header)
        .children(state.notice().map(|notice| {
            div()
                .px(px(SPACE_5))
                .pt(px(SPACE_4))
                .child(note(notice.to_owned(), &theme))
        }))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .mt(px(SPACE_4))
                .debug_selector(|| "board-columns".to_owned())
                .child(board_columns(model, shell, owner, SPACE_5, window, cx)),
        )
        .into_any_element()
}

/// The four columns of a Board, each with its cards in order (4.3). The row scrolls sideways in a
/// ScrollArea; `inset` keeps the first and last column clear of the edges it scrolls to.
pub(in crate::shell) fn board_columns(
    model: &ShellModel,
    shell: &WeakEntity<Shell>,
    owner: BoardOwner,
    inset: f32,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = *cx.theme();
    let Some(view) = model.teams().board_view(owner) else {
        return note("This Board is gone.", &theme).into_any_element();
    };
    let mut columns = div().flex().gap(px(SPACE_3)).h_full().px(px(inset));
    for column in view.columns {
        let mut built = BoardColumn::new(
            format!("board-column-{}", column.column.title()),
            column.column.title(),
        );
        for card in column.cards {
            let focus = row_focus(window, cx, format!("board-card-{}", card.id.0));
            built = built.card(board_card(&card, column.column, owner, shell, focus));
        }
        columns = columns.child(built);
    }
    div()
        .flex_1()
        .min_h_0()
        .w_full()
        .child(
            ScrollArea::new("board-columns")
                .axis(ScrollAxis::Horizontal)
                .child(columns),
        )
        .into_any_element()
}

/// One card: its status glyph and word, the bot (or Unassigned), the time, and its key handler.
fn board_card(
    card: &CardView,
    column: Column,
    owner: BoardOwner,
    shell: &WeakEntity<Shell>,
    focus: FocusHandle,
) -> BoardCard {
    let built = BoardCard::new(format!("task-{}", card.id.0), card.title.clone(), focus)
        .lifted(card.lifted);
    let mut built = match card.bot.as_deref() {
        Some(bot) => built.bot(bot),
        None => built.unassigned(),
    };
    if let Some((state, word)) = status_of(column) {
        built = built.status(state, word);
    }
    if let Some(reason) = card.kind_unavailable {
        built = built.unavailable(reason);
    }
    if let Some(time) = &card.time {
        built = built.time(time.clone());
    }
    if let Some(note) = &card.note {
        built = built.note(note.clone());
    }
    if let HostReach::Unreachable { minutes } = card.reach {
        built = built.unreachable(format!("Last known state, {minutes} min ago"));
    }
    let (shell, task) = (shell.clone(), card.id);
    built.on_key_down(move |event, window, cx| {
        let key = event.keystroke.key.as_str();
        if !PICK_KEYS.contains(&key) {
            return;
        }
        let handled = shell
            .update(cx, |shell, cx| shell.board_key(owner, task, key, cx))
            .unwrap_or(false);
        if handled {
            window.prevent_default();
        }
    })
}

/// The status a card shows in each column. Open cards are unassigned and show none.
fn status_of(column: Column) -> Option<(GlyphState, &'static str)> {
    match column {
        Column::Open => None,
        Column::Running => Some((GlyphState::Running, "Running")),
        Column::Waiting => Some((GlyphState::Waiting, "Waiting for you")),
        Column::Done => Some((GlyphState::Done, "Done")),
    }
}

fn note(text: impl Into<String>, theme: &Theme) -> impl IntoElement {
    let text: String = text.into();
    div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.tertiary)
        .child(text)
}

impl Shell {
    /// Sends the call a drop asks for: start, stop, finish or reopen. The card moves when the read that follows the
    /// service's answer lands; a refusal shows on the Board until the next pick.
    fn drop_live_pick(&mut self, service: ServiceHandle, cx: &mut Context<Self>) {
        let Some(pick) = self.model.teams_mut().take_pick() else {
            return;
        };
        let from = self
            .model
            .teams()
            .tasks(pick.board)
            .and_then(|tasks| tasks.iter().find(|task| task.id == pick.task))
            .map(|task| task.column);
        let key = self
            .store
            .read(cx)
            .state()
            .team_ids
            .key(pick.task.0)
            .map(str::to_owned);
        let call = from.map_or(BoardMove::Stay, |from| board_move(from, pick.column));
        match (call, key) {
            (BoardMove::Stay, _) => {}
            (BoardMove::Refused(reason), _) => {
                self.model.teams_mut().set_notice(Some(reason.to_owned()))
            }
            (_, None) => self
                .model
                .teams_mut()
                .set_notice(Some(TeamError::Missing.message().to_owned())),
            (BoardMove::Start, Some(task)) => {
                self.answer_drop(cx, async move { service.start_task(task).await });
            }
            (BoardMove::Stop, Some(task)) => {
                self.answer_drop(cx, async move { service.stop_task(task).await });
            }
            (BoardMove::Finish, Some(task)) => {
                self.answer_drop(cx, async move { service.finish_task(task).await });
            }
            (BoardMove::Reopen, Some(task)) => {
                self.answer_drop(cx, async move { service.reopen_task(task).await });
            }
        }
        cx.notify();
    }

    /// Waits for the service's answer to a drop; a refusal shows on the Board until the next pick.
    fn answer_drop(
        &mut self,
        cx: &mut Context<Self>,
        call: impl Future<Output = Result<Task, Refusal>> + 'static,
    ) {
        cx.spawn(async move |this, cx| {
            if let Err(refusal) = call.await {
                this.update(cx, |shell, cx| {
                    shell.model.teams_mut().set_notice(Some(refusal.message()));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Handles one key for the Board: it starts, moves, drops or cancels a pick. Returns whether the
    /// key was used, so the others (Escape closing an overlay) keep working when no card is picked.
    pub(super) fn board_key(
        &mut self,
        owner: BoardOwner,
        task: TaskId,
        key: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if key == "space"
            && let Some(service) = self.service.clone()
            && self.model.teams().pick().is_some()
        {
            self.drop_live_pick(service, cx);
            return true;
        }
        let teams = self.model.teams_mut();
        let picked = teams.pick().is_some();
        let handled = match key {
            "space" if picked => teams.drop_pick().is_ok(),
            "space" => teams.pick_task(owner, task).is_ok(),
            "left" if picked => teams.step_pick(PickStep::Left),
            "right" if picked => teams.step_pick(PickStep::Right),
            "up" if picked => teams.step_pick(PickStep::Up),
            "down" if picked => teams.step_pick(PickStep::Down),
            "escape" if picked => {
                teams.cancel_pick();
                true
            }
            _ => false,
        };
        cx.notify();
        handled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::SampleDataSource;
    use crate::model::teams::{SampleTeamsSource, TeamId, TeamPage};
    use gpui::{Entity, TestAppContext, VisualTestContext, size};
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

    fn key(shell: &Entity<Shell>, cx: &mut VisualTestContext, name: &'static str) -> bool {
        shell.update(cx, |shell, cx| {
            shell.board_key(BoardOwner::Team(TeamId(1)), TaskId(1), name, cx)
        })
    }

    #[gpui::test]
    fn the_board_header_is_one_row_and_the_columns_start_where_bt_board_puts_them(
        cx: &mut TestAppContext,
    ) {
        let (shell, cx) = shell(cx);
        cx.simulate_resize(size(px(1440.), px(900.)));
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            shell.model.open_team_page(TeamPage::Board(TeamId(1)));
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let canvas = cx.debug_bounds("canvas").expect("the canvas is drawn");
        let header = cx
            .debug_bounds("board-header")
            .expect("the header is drawn");
        let columns = cx
            .debug_bounds("board-columns")
            .expect("the columns are drawn");
        // bt-board: the canvas starts under the 36 px title bar, the header is 56 px and the
        // columns start 16 px below it, so the first column sits at y 108 at 1440 by 900.
        assert_eq!(header.origin.y, canvas.origin.y);
        assert_eq!(header.size.height, px(56.));
        assert_eq!(columns.origin.y, px(108.));
    }

    #[gpui::test]
    fn the_team_pill_draws_the_team_character(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            shell.model.open_team_page(TeamPage::Board(TeamId(1)));
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(
            cx.debug_bounds("team-character").is_some(),
            "the team pill draws the team's character, not the users icon"
        );
    }

    #[gpui::test]
    fn the_team_board_moves_a_card_with_the_keys_and_drops_it(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            shell.model.open_team_page(TeamPage::Board(TeamId(1)));
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(!key(&shell, cx, "right"), "no card is picked up yet");
        assert!(key(&shell, cx, "space"), "Space picks the first card up");
        assert!(key(&shell, cx, "right"));
        assert!(key(&shell, cx, "space"), "Space drops it in Running");
        shell.read_with(cx, |shell, _| {
            let task = shell.model().teams().team(TeamId(1)).unwrap().tasks[0].clone();
            assert_eq!(task.column, Column::Running);
            assert!(shell.model().teams().pick().is_none());
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    #[gpui::test]
    fn escape_puts_a_picked_card_back_and_leaves_the_board_as_it_was(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            shell.model.open_team_page(TeamPage::Board(TeamId(1)));
            cx.notify();
        });
        assert!(key(&shell, cx, "space"));
        assert!(key(&shell, cx, "right"));
        assert!(key(&shell, cx, "escape"));
        shell.read_with(cx, |shell, _| {
            let task = shell.model().teams().team(TeamId(1)).unwrap().tasks[0].clone();
            assert_eq!(task.column, Column::Open);
        });
    }

    #[gpui::test]
    fn an_open_task_with_no_bot_shows_the_unassigned_chip(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            shell.model.open_team_page(TeamPage::Board(TeamId(1)));
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(
            cx.debug_bounds("board-card-unassigned").is_some(),
            "the open task has no bot, so its card shows the Unassigned chip"
        );
        assert!(
            cx.debug_bounds("board-card-bot").is_some(),
            "a card with a bot still names it"
        );
    }
}
