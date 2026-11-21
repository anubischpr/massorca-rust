//! Gallery stories for the lists group.

use super::{Story, variant};
use crate::components::agent_icon::{AgentIconSize, AgentKind};
use crate::components::agent_status::{AgentState, AgentStatusSize};
use crate::components::badge::Badge;
use crate::components::button::StatePreview;
use crate::components::icon_button::IconButton;
use crate::components::list_row::{ListRow, RowLeading, RowLines, TREE_INDENT};
use crate::components::segmented_control::{
    Segment, SegmentedControl, SegmentedSize, SegmentedStyle,
};
use crate::components::sidebar_row::{HostConnection, SidebarRow, WorkspaceKind};
use crate::components::usage_ring::UsageSlot;
use crate::icon::IconName;
use crate::theme::ActiveTheme;
use crate::tokens::{SPACE_2, SPACE_4};
use gpui::{AnyElement, App, Div, IntoElement, ParentElement, Styled, Window, div, px};

pub fn stories() -> Vec<Story> {
    vec![
        Story {
            name: "List row",
            spec: "ListRow.md",
            render: list_row,
        },
        Story {
            name: "Sidebar row",
            spec: "SidebarRow.md",
            render: sidebar_row,
        },
        Story {
            name: "Segmented control",
            spec: "SegmentedControl.md",
            render: segmented_control,
        },
    ]
}

/// A column of rows, 300 px wide, so truncation shows.
fn list() -> Div {
    div().flex().flex_col().w(px(300.)).gap(px(2.))
}

/// A Sidebar-width column on the Frame surface, where sidebar rows sit.
fn sidebar(cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .w(px(256.))
        .p(px(SPACE_2))
        .gap(px(2.))
        .bg(cx.theme().palette.surface.frame)
}

fn column() -> Div {
    div().flex().flex_col().gap(px(SPACE_4))
}

fn slot(index: u8) -> UsageSlot {
    UsageSlot::new(index).unwrap_or_default()
}

fn sidebar_row(_: &mut Window, cx: &mut App) -> AnyElement {
    let session_indent = 3.0 * TREE_INDENT;
    column()
        .child(variant(
            "Navigation entries: a count and a chevron when they have children",
            sidebar(cx)
                .child(
                    SidebarRow::nav("sb-proto", IconName::LayoutGrid, "Prototypes")
                        .count(2)
                        .expanded(false),
                )
                .child(SidebarRow::nav(
                    "sb-tasks",
                    IconName::ListChecks,
                    "Tasks",
                ))
                .child(SidebarRow::nav(
                    "sb-auto",
                    IconName::Workflow,
                    "Automations",
                ))
                .child(SidebarRow::nav(
                    "sb-mobile",
                    IconName::Smartphone,
                    "MassOrca Mobile",
                )),
            cx,
        ))
        .child(variant(
            "Project, hosts and a workspace: a reachable host, then one that cannot be reached",
            sidebar(cx)
                .child(
                    SidebarRow::project("sb-project", IconName::Folder, "massorca")
                        .secondary("acme-labs"),
                )
                .child(
                    SidebarRow::host("sb-host-a", "This PC", HostConnection::Connected).level(1),
                )
                .child(
                    SidebarRow::host("sb-host-b", "oppm-api", HostConnection::Unreachable)
                        .level(1),
                )
                .child(
                    SidebarRow::workspace(
                        "sb-ws-a",
                        "feature/lists",
                        WorkspaceKind::Worktree,
                        slot(1),
                    )
                    .level(2),
                ),
            cx,
        ))
        .child(variant(
            "Workspace labels take the six usage colours (P29)",
            sidebar(cx)
                .child(
                    SidebarRow::workspace("sb-u1", "usage 1", WorkspaceKind::Worktree, slot(1))
                        .level(2),
                )
                .child(
                    SidebarRow::workspace("sb-u2", "usage 2", WorkspaceKind::Folder, slot(2))
                        .level(2),
                )
                .child(
                    SidebarRow::workspace("sb-u3", "usage 3", WorkspaceKind::Worktree, slot(3))
                        .level(2),
                )
                .child(
                    SidebarRow::workspace("sb-u4", "usage 4", WorkspaceKind::Worktree, slot(4))
                        .level(2),
                )
                .child(
                    SidebarRow::workspace("sb-u5", "usage 5", WorkspaceKind::Folder, slot(5))
                        .level(2),
                )
                .child(
                    SidebarRow::workspace("sb-u6", "usage 6", WorkspaceKind::Worktree, slot(6))
                        .level(2),
                ),
            cx,
        ))
        .child(variant(
            "Sessions: status glyph, agent mark, then title; the child's mark lines up with the parent's title",
            sidebar(cx)
                .child(
                    SidebarRow::session(
                        "sb-s1",
                        "Feed endpoints",
                        AgentState::Running,
                        AgentKind::Claude,
                    )
                    .level(3)
                    .hover_action(IconButton::new(
                        "sb-s1-more",
                        IconName::Ellipsis,
                        "Session actions",
                    )),
                )
                .child(
                    SidebarRow::session(
                        "sb-s2",
                        "Review the diff",
                        AgentState::Waiting,
                        AgentKind::Codex,
                    )
                    .child_of(session_indent),
                )
                .child(
                    SidebarRow::session(
                        "sb-s3",
                        "Could not start",
                        AgentState::Error,
                        AgentKind::Gemini,
                    )
                    .level(3)
                    .error("Failed to start"),
                ),
            cx,
        ))
        .child(variant(
            "States: selected, hover, pressed, focus, loading and a count",
            sidebar(cx)
                .child(
                    SidebarRow::nav("sb-selected", IconName::ListChecks, "Tasks")
                        .selected(true),
                )
                .child(
                    SidebarRow::nav("sb-hover", IconName::Workflow, "Hover")
                        .preview(StatePreview::Hover),
                )
                .child(
                    SidebarRow::nav("sb-pressed", IconName::Workflow, "Pressed")
                        .preview(StatePreview::Pressed),
                )
                .child(
                    SidebarRow::nav("sb-focus", IconName::Workflow, "Focus, from the keyboard")
                        .preview(StatePreview::Focus),
                )
                .child(SidebarRow::new("sb-loading", "Loading").loading(true))
                .child(
                    SidebarRow::nav("sb-count", IconName::Inbox, "Inbox")
                        .count(12),
                ),
            cx,
        ))
        .into_any_element()
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

/// A row of items that wraps.
fn row() -> Div {
    div().flex().flex_wrap().items_center().gap(px(SPACE_2))
}

fn segmented_control(_: &mut Window, cx: &mut App) -> AnyElement {
    let two = || vec![Segment::text("One"), Segment::text("Two")];
    column()
        .child(variant(
            "Outlined, text: two and three options",
            row()
                .child(SegmentedControl::new(
                    "seg-search",
                    "Search in",
                    vec![Segment::text("Names"), Segment::text("Content")],
                ))
                .child(
                    SegmentedControl::new(
                        "seg-days",
                        "View",
                        vec![
                            Segment::text("Day"),
                            Segment::text("Week"),
                            Segment::text("Month"),
                        ],
                    )
                    .selected(1),
                ),
            cx,
        ))
        .child(variant(
            "Icon and text, and icon only (each with a name and a tooltip)",
            row()
                .child(SegmentedControl::new(
                    "seg-icon-text",
                    "View",
                    vec![
                        Segment::icon_text(IconName::List, "List"),
                        Segment::icon_text(IconName::Kanban, "Board"),
                    ],
                ))
                .child(
                    SegmentedControl::new(
                        "seg-icons",
                        "Show",
                        vec![
                            Segment::icon_only(IconName::List, "List"),
                            Segment::icon_only(IconName::Kanban, "Board"),
                            Segment::icon_only(IconName::Calendar, "Calendar"),
                        ],
                    )
                    .selected(2),
                ),
            cx,
        ))
        .child(variant(
            "Sunken, on a composer (A30): Chat and Terminal",
            row().child(
                SegmentedControl::new(
                    "seg-chat",
                    "Mode",
                    vec![
                        Segment::icon_text(IconName::MessageSquare, "Chat"),
                        Segment::icon_text(IconName::SquareTerminal, "Terminal"),
                    ],
                )
                .style(SegmentedStyle::Sunken),
            ),
            cx,
        ))
        .child(variant(
            "Sizes: compact, default and comfortable",
            row()
                .child(
                    SegmentedControl::new("seg-compact", "Size", two())
                        .size(SegmentedSize::Compact),
                )
                .child(SegmentedControl::new("seg-default", "Size", two()))
                .child(
                    SegmentedControl::new("seg-comfortable", "Size", two())
                        .size(SegmentedSize::Comfortable),
                ),
            cx,
        ))
        .child(variant(
            "Strong selected style, at most once per view",
            row().child(
                SegmentedControl::new(
                    "seg-strong",
                    "Layout",
                    vec![Segment::text("Split"), Segment::text("Unified")],
                )
                .strong(true),
            ),
            cx,
        ))
        .child(variant(
            "States: hover, pressed, focus, a disabled segment and a disabled control",
            row()
                .child(
                    SegmentedControl::new("seg-hover", "Hover", two())
                        .preview(1, StatePreview::Hover),
                )
                .child(
                    SegmentedControl::new("seg-pressed", "Pressed", two())
                        .preview(1, StatePreview::Pressed),
                )
                .child(
                    SegmentedControl::new("seg-focus", "Focus", two())
                        .preview(0, StatePreview::Focus),
                )
                .child(SegmentedControl::new(
                    "seg-off-segment",
                    "Disabled segment",
                    vec![Segment::text("On"), Segment::text("Off").disabled(true)],
                ))
                .child(SegmentedControl::new("seg-disabled", "Disabled", two()).disabled(true)),
            cx,
        ))
        .into_any_element()
}
