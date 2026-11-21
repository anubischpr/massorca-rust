//! Gallery stories for the Bot Teams components (screens 4).

use super::{Story, variant};
use crate::components::agent_status::AgentState;
use crate::components::board_column::{BoardCard, BoardColumn};
use crate::tokens::{SPACE_4, SPACE_6};
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, px};

pub fn stories() -> Vec<Story> {
    vec![Story {
        name: "Board column",
        spec: "BoardColumn.md",
        render: board_column,
    }]
}

fn card(cx: &mut App, id: &'static str, title: &'static str) -> BoardCard {
    BoardCard::new(id, title, cx.focus_handle())
}

fn board_column(_: &mut Window, cx: &mut App) -> AnyElement {
    let running = [
        card(cx, "run-1", "Magic link sign-in for oppm")
            .bot("Mara")
            .status(AgentState::Running, "Running")
            .time("09:42")
            .note("Two bots delegated"),
        card(cx, "run-2", "Nightly summary to the team")
            .bot("Bruno")
            .status(AgentState::Running, "Running")
            .unreachable("Last known state, 5 min ago"),
    ];
    let waiting = card(cx, "wait-1", "Approve the magic link email template")
        .bot("Iris")
        .status(AgentState::Waiting, "Waiting for you")
        .note("Needs your reply in the Conference room");
    let lifted = card(cx, "lift-1", "Write release notes for oppm")
        .status(AgentState::Idle, "Idle")
        .lifted(true);
    let default = div()
        .flex()
        .gap(px(SPACE_4))
        .child(BoardColumn::new("column-open", "Open").card(
            card(cx, "open-1", "Write release notes for oppm").status(AgentState::Idle, "Idle"),
        ))
        .child(BoardColumn::new("column-running", "Running").cards(running))
        .child(BoardColumn::new("column-waiting", "Waiting for you").card(waiting))
        .child(BoardColumn::new("column-done", "Done"));
    let picked = div()
        .flex()
        .gap(px(SPACE_4))
        .child(BoardColumn::new("picked-open", "Open"))
        .child(BoardColumn::new("picked-running", "Running").card(lifted));
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_6))
        .child(variant(
            "Four columns with cards and empty Done",
            default,
            cx,
        ))
        .child(variant("A card picked up for a move", picked, cx))
        .into_any_element()
}
