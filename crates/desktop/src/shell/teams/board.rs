//! The Board (D-WS-30, screens 4.3): four columns of cards. Space picks a card up, the arrow keys move
//! it between columns and places, Space drops it and Escape puts it back. The Board changes only on
//! the drop, and a move never starts or stops an agent.

use super::super::Shell;
use super::super::sidebar::row_focus;
use super::team_pill;
use crate::model::settings_sections::NOT_AVAILABLE;
use crate::model::shell::HostReach;
use crate::model::shell_state::ShellModel;
use crate::model::teams::{BoardOwner, CardView, Column, PickStep, TaskId};
use crate::service::{Refusal, ServiceHandle};
use gpui::{
    AnyElement, App, Context, FocusHandle, ParentElement, Styled, WeakEntity, Window, div,
    prelude::*, px,
};
use massorca_protocol::methods::tasks::Task;
use massorca_ui::components::agent_status::AgentState as GlyphState;
use massorca_ui::components::board_column::{BoardCard, BoardColumn};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::scroll_area::{ScrollArea, ScrollAxis};
use massorca_ui::tokens::{SPACE_2, SPACE_4, SPACE_6, TYPE_BODY_SM, TYPE_HEADING};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};
use std::future::Future;

/// The keys a picked card reads: Space, the arrows and Escape.
const PICK_KEYS: [&str; 6] = ["space", "left", "right", "up", "down", "escape"];

/// The Board page of a team: its header, the team pill and the four columns (4.3).
/// What a refused move says: the Board only moves cards to Done, and back to Open from Done.
const MOVE_NOT_AVAILABLE: &str = "This move is not available yet.";

pub(in crate::shell) fn board_page(
    model: &ShellModel,
    shell: &WeakEntity<Shell>,
    owner: BoardOwner,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let theme = *cx.theme();
    let state = model.teams();
    let team = match owner {
        BoardOwner::Team(team) | BoardOwner::Project(team, _) => team,
    };
    let team_name = state.team(team).map_or("", |team| team.name.as_str());
    let header = div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_style(TYPE_HEADING)
                .text_color(theme.palette.text.primary)
                .child("Board"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(SPACE_2))
                .child(team_pill(team_name, team, &theme))
                .child(
                    Button::new("board-filter", "Filter")
                        .variant(ButtonVariant::Ghost)
                        .leading_icon(IconName::ListFilter)
                        .disabled(true)
                        .tooltip(NOT_AVAILABLE),
                ),
        );
    // The header keeps the page inset; the columns run to the window edge and scroll there.
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .size_full()
        .pt(px(SPACE_6))
        .pb(px(SPACE_6))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(SPACE_4))
                .px(px(SPACE_6))
                .child(header)
                .child(note("Tasks come from the Conference room", &theme))
                .children(state.notice().map(|notice| note(notice.to_owned(), &theme))),
        )
        .child(board_columns(model, shell, owner, SPACE_6, window, cx))
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
    let mut columns = div().flex().gap(px(SPACE_2)).h_full().px(px(inset));
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
    let mut built = BoardCard::new(format!("task-{}", card.id.0), card.title.clone(), focus)
        .lifted(card.lifted)
        .bot(card.bot.clone().unwrap_or_else(|| "Unassigned".into()));
    if let Some((state, word)) = status_of(column) {
        built = built.status(state, word);
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
    /// Drops the picked card on the service. Only a move to Done, or back to Open from Done, has a call; any
    /// other move says so. The card moves when the read that follows the service's answer lands.
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
        match (from, pick.column, key) {
            (Some(from), to, _) if from == to => {}
            (_, Column::Done, Some(task)) => {
                self.answer_drop(cx, async move { service.finish_task(task).await });
            }
            (Some(Column::Done), Column::Open, Some(task)) => {
                self.answer_drop(cx, async move { service.reopen_task(task).await });
            }
            _ => self
                .model
                .teams_mut()
                .set_notice(Some(MOVE_NOT_AVAILABLE.to_owned())),
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

    fn key(shell: &Entity<Shell>, cx: &mut VisualTestContext, name: &'static str) -> bool {
        shell.update(cx, |shell, cx| {
            shell.board_key(BoardOwner::Team(TeamId(1)), TaskId(1), name, cx)
        })
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
}
