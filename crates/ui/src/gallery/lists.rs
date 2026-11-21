//! Gallery stories for the lists group.

use super::{Story, variant};
use crate::components::agent_icon::{AgentIconSize, AgentKind};
use crate::components::agent_status::{AgentState, AgentStatusSize};
use crate::components::badge::Badge;
use crate::components::button::StatePreview;
use crate::components::icon_button::IconButton;
use crate::components::list_row::{ListRow, RowLeading, RowLines};
use crate::icon::IconName;
use crate::tokens::SPACE_4;
use gpui::{AnyElement, App, Div, IntoElement, ParentElement, Styled, Window, div, px};

pub fn stories() -> Vec<Story> {
    vec![Story {
        name: "List row",
        spec: "ListRow.md",
        render: list_row,
    }]
}

/// A column of rows, 300 px wide, so truncation shows.
fn list() -> Div {
    div().flex().flex_col().w(px(300.)).gap(px(2.))
}

fn column() -> Div {
    div().flex().flex_col().gap(px(SPACE_4))
}

fn list_row(_: &mut Window, cx: &mut App) -> AnyElement {
    column()
        .child(variant(
            "One line: rest, selected, and the hover, pressed and focus states",
            list()
                .child(
                    ListRow::new("lr-rest", "Design tokens")
                        .leading(RowLeading::Icon(IconName::FileText))
                        .secondary("12 KB"),
                )
                .child(
                    ListRow::new("lr-selected", "Components")
                        .leading(RowLeading::Icon(IconName::Folder))
                        .selected(true),
                )
                .child(
                    ListRow::new("lr-hover", "Hover")
                        .leading(RowLeading::Icon(IconName::File))
                        .preview(StatePreview::Hover),
                )
                .child(
                    ListRow::new("lr-pressed", "Pressed")
                        .leading(RowLeading::Icon(IconName::File))
                        .preview(StatePreview::Pressed),
                )
                .child(
                    ListRow::new("lr-focus", "Focus, from the keyboard")
                        .leading(RowLeading::Icon(IconName::File))
                        .preview(StatePreview::Focus),
                ),
            cx,
        ))
        .child(variant(
            "States: disabled, loading, error and dimmed",
            list()
                .child(
                    ListRow::new("lr-disabled", "Disabled row")
                        .leading(RowLeading::Icon(IconName::File))
                        .disabled(true),
                )
                .child(ListRow::new("lr-loading", "Loading").loading(true))
                .child(
                    ListRow::new("lr-error", "Shared folder")
                        .leading(RowLeading::Icon(IconName::Folder))
                        .error("Could not read the folder"),
                )
                .child(
                    ListRow::new("lr-dimmed", "Oppm-api")
                        .leading(RowLeading::Status(
                            AgentState::Unreachable,
                            AgentStatusSize::Small,
                        ))
                        .secondary("Last known state, 5 min ago")
                        .lines(RowLines::Two)
                        .dimmed(true),
                ),
            cx,
        ))
        .child(variant(
            "Two lines, with an agent mark and a trailing badge",
            list().child(
                ListRow::new("lr-two", "Feed endpoints")
                    .lines(RowLines::Two)
                    .leading(RowLeading::Agent(AgentKind::Claude, AgentIconSize::Small))
                    .secondary("main, 2 changes")
                    .trailing(Badge::count(2)),
            ),
            cx,
        ))
        .child(variant(
            "Tree levels with disclosure chevrons and a status glyph",
            list()
                .child(
                    ListRow::new("lr-tree-0", "massorca")
                        .leading(RowLeading::Icon(IconName::Folder))
                        .expanded(true),
                )
                .child(
                    ListRow::new("lr-tree-1", "feature/lists")
                        .level(1)
                        .leading(RowLeading::Icon(IconName::GitBranch))
                        .expanded(false),
                )
                .child(
                    ListRow::new("lr-tree-2", "Waiting for you")
                        .level(2)
                        .leading(RowLeading::Status(
                            AgentState::Waiting,
                            AgentStatusSize::Small,
                        )),
                ),
            cx,
        ))
        .child(variant(
            "Hover action (shown on hover or focus), and a long title that truncates",
            list()
                .child(
                    ListRow::new("lr-action", "Session with a trailing action")
                        .trailing(Badge::count(1))
                        .hover_action(IconButton::new(
                            "lr-action-more",
                            IconName::Ellipsis,
                            "More actions",
                        )),
                )
                .child(
                    ListRow::new(
                        "lr-long",
                        "A session title that is much longer than the row and keeps going",
                    )
                    .tooltip("A session title that is much longer than the row and keeps going")
                    .leading(RowLeading::Agent(AgentKind::Codex, AgentIconSize::Small)),
                ),
            cx,
        ))
        .into_any_element()
}
