//! Stories of the data components: Table, and later FileTree and Picker. A stateful
//! component keeps its entity across frames under a key of its own.

use super::{Story, variant};
use crate::components::agent_icon::AgentKind;
use crate::components::agent_status::AgentState;
use crate::components::table::{
    SortDirection, TableCell, TableColumn, TableDensity, TableRow, TableState, TableStatus,
    TableView,
};
use crate::icon::IconName;
use crate::tokens::SPACE_4;
use gpui::{AnyElement, App, Context, Entity, IntoElement, ParentElement, Styled, Window, div, px};

/// Height of a gallery frame around a live component. A layout size of the gallery, not a token.
const FRAME_HEIGHT: f32 = 180.0;

pub fn stories() -> Vec<Story> {
    vec![Story {
        name: "Table",
        spec: "Table.md",
        render: table,
    }]
}

fn table(window: &mut Window, cx: &mut App) -> AnyElement {
    let default = keyed_table(window, cx, "table-default", |cx| {
        TableView::new("table-default", "Checks", checks(), cx)
    });
    let sorted = keyed_table(window, cx, "table-sorted", |cx| {
        let state = checks().sorted_by(3, SortDirection::Descending);
        TableView::new("table-sorted", "Checks sorted by duration", state, cx)
    });
    let selected = keyed_table(window, cx, "table-selected", |cx| {
        let mut state = checks().selectable();
        state.select(&"lint".into());
        TableView::new("table-selected", "Checks with a selection", state, cx)
            .density(TableDensity::Comfortable)
    });
    let loading = keyed_table(window, cx, "table-loading", |cx| {
        let state = with_status(TableStatus::Loading);
        TableView::new("table-loading", "Checks loading", state, cx)
    });
    let empty = keyed_table(window, cx, "table-empty", |cx| {
        let state = with_status(TableStatus::Empty {
            title: "No check runs for this branch.".into(),
            explanation: Some("Checks appear after the first push.".into()),
            action: Some("Refresh".into()),
        });
        TableView::new("table-empty", "Checks empty", state, cx)
    });
    let error = keyed_table(window, cx, "table-error", |cx| {
        let state = with_status(TableStatus::Error);
        TableView::new("table-error", "Checks error", state, cx)
    });

    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_4))
        .child(variant(
            "Default, with a long name that truncates",
            frame(default),
            cx,
        ))
        .child(variant("Sorted by duration, descending", frame(sorted), cx))
        .child(variant(
            "Selectable, comfortable rows, one row selected",
            frame(selected),
            cx,
        ))
        .child(variant("Loading: five skeleton rows", frame(loading), cx))
        .child(variant("Empty, with one action", frame(empty), cx))
        .child(variant("Error, with Retry", frame(error), cx))
        .into_any_element()
}

/// The table of one variant, kept across frames under its own key.
fn keyed_table(
    window: &mut Window,
    cx: &mut App,
    key: &'static str,
    build: fn(&mut Context<TableView>) -> TableView,
) -> Entity<TableView> {
    window.use_keyed_state(key, cx, |_, cx| build(cx))
}

fn frame(table: Entity<TableView>) -> impl IntoElement {
    div().h(px(FRAME_HEIGHT)).child(table)
}

fn with_status(status: TableStatus) -> TableState {
    let mut state = checks();
    state.set_status(status);
    state
}

/// Checks of one branch. The last name is long enough to truncate.
fn checks() -> TableState {
    TableState::new(
        vec![
            TableColumn::new("Name").sortable(),
            TableColumn::new("Agent"),
            TableColumn::new("Status"),
            TableColumn::new("Duration").numeric().sortable(),
            TableColumn::new("Log"),
        ],
        vec![
            check(
                "build",
                "Build",
                AgentKind::Claude,
                AgentState::Running,
                72.0,
                "1m 12s",
            ),
            check(
                "lint",
                "Lint",
                AgentKind::Codex,
                AgentState::Done,
                14.0,
                "14s",
            ),
            check(
                "endpoints",
                "Feed endpoints for the workspace status checks, including the slow ones",
                AgentKind::Gemini,
                AgentState::Waiting,
                301.0,
                "5m 1s",
            ),
            check(
                "sync",
                "Sync",
                AgentKind::Codex,
                AgentState::Unreachable,
                0.0,
                "0s",
            ),
        ],
    )
}

fn check(
    key: &str,
    name: &str,
    agent: AgentKind,
    state: AgentState,
    seconds: f64,
    duration: &str,
) -> TableRow {
    TableRow::new(
        key,
        vec![
            TableCell::Text(name.into()),
            TableCell::Agent(agent),
            TableCell::Status(state),
            TableCell::Numeric {
                value: seconds,
                text: duration.into(),
            },
            TableCell::Action {
                icon: IconName::ExternalLink,
                name: "Open log".into(),
                disabled: false,
            },
        ],
    )
}
