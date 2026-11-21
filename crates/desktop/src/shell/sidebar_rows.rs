//! Rows of the Sidebar: the navigation rows and the project tree. Kept apart from the Sidebar
//! so the row design can move to the shared SidebarRow later (A10, A83 to A85, A84).

use super::Shell;
use super::change;
use super::rail::agent_kind;
use crate::model::shell::{AgentState, HostReach};
use crate::model::sidebar::{NavKind, NavRow, TreeRow};
use gpui::{
    AnyElement, Div, FocusHandle, FontWeight, IntoElement, SharedString, Stateful, WeakEntity, div,
    prelude::*, px,
};
use massorca_ui::components::agent_icon::{AgentIcon, AgentIconSize};
use massorca_ui::components::agent_status::{
    AgentState as GlyphState, AgentStatus, AgentStatusSize,
};
use massorca_ui::style::{FocusRing, Surface};
use massorca_ui::tokens::{
    Color, RADIUS_FULL, RADIUS_MD, SIZE_CONTROL_DEFAULT, SIZE_INDICATOR_DOT, SPACE_2, SPACE_4,
    SPACE_6, TYPE_BODY, TYPE_BODY_STRONG, TYPE_CAPTION,
};
use massorca_ui::{Icon, IconName, IconSize, TextStyled, Theme};

/// Left inset of each level. A session's agent icon lands under its parent's title, because a
/// child is inset by one `CHILD_INSET` (A85).
const PROJECT_INSET: f32 = SPACE_2;
const HOST_INSET: f32 = SPACE_2 + SPACE_4;
const WORKSPACE_INSET: f32 = SPACE_2 + 2.0 * SPACE_4;
const CHILD_INSET: f32 = SPACE_6;

/// A navigation row: an icon, its label and an optional count. A row with a page opens it.
pub(super) fn nav_row(
    row: &NavRow,
    focus: &FocusHandle,
    shell: &WeakEntity<Shell>,
    theme: &Theme,
) -> AnyElement {
    let fills = Surface::Frame.fills(theme);
    let tone = theme.palette.text.secondary;
    let mut item = div()
        .id(row.label)
        .track_focus(focus)
        .tab_stop(true)
        .focus_ring(theme)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(SPACE_2))
        .h(px(SIZE_CONTROL_DEFAULT))
        .px(px(SPACE_2))
        .rounded(px(RADIUS_MD))
        .text_style(TYPE_BODY)
        .text_color(tone)
        .hover(move |style| style.bg(fills.hover))
        .active(move |style| style.bg(fills.pressed))
        .child(icon(nav_icon(row.kind), tone))
        .child(label(row.label));
    if let Some(count) = row.count {
        item = item.child(
            div()
                .flex_none()
                .text_style(TYPE_CAPTION)
                .text_color(theme.palette.text.tertiary)
                .child(count.to_string()),
        );
    }
    if let Some(page) = row.page {
        let shell = shell.clone();
        item = item.cursor_pointer().on_click(move |_, _, cx| {
            change(&shell, cx, move |shell| shell.model.open_page(page));
        });
    }
    item.into_any_element()
}

/// One row of the project tree, identified by its position in the list.
pub(super) fn tree_row(
    row: &TreeRow,
    index: usize,
    focus: &FocusHandle,
    theme: &Theme,
) -> AnyElement {
    let id = SharedString::from(format!("tree-row-{index}"));
    match row {
        TreeRow::Project { name } => row_frame(id, focus, PROJECT_INSET, false, theme)
            .text_style(TYPE_BODY_STRONG)
            .text_color(theme.palette.text.primary)
            .child(icon(IconName::Folder, theme.palette.text.secondary))
            .child(label(name.clone()))
            .into_any_element(),
        TreeRow::Host { name, reach } => host_row(name, *reach, id, focus, theme),
        TreeRow::Workspace { name, branch } => {
            let mut item = row_frame(id, focus, WORKSPACE_INSET, false, theme)
                .child(icon(IconName::GitBranch, theme.palette.text.tertiary))
                .child(label(name.clone()));
            if let Some(branch) = branch {
                item = item.child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_style(TYPE_CAPTION)
                        .text_color(theme.palette.text.tertiary)
                        .child(branch.clone()),
                );
            }
            item.into_any_element()
        }
        TreeRow::Session {
            title,
            agent,
            state,
            depth,
            selected,
        } => {
            let inset = WORKSPACE_INSET + *depth as f32 * CHILD_INSET;
            let tone = if *selected {
                theme.palette.text.primary
            } else {
                theme.palette.text.secondary
            };
            row_frame(id, focus, inset, *selected, theme)
                .text_color(tone)
                .child(AgentStatus::new(glyph_state(*state)).size(AgentStatusSize::Small))
                .child(AgentIcon::new(agent_kind(*agent)).size(AgentIconSize::Small))
                .child(label(title.clone()))
                .into_any_element()
        }
    }
}

/// A host row. An unreachable host dims only itself and shows how long ago it was reached (A10).
fn host_row(
    name: &str,
    reach: HostReach,
    id: SharedString,
    focus: &FocusHandle,
    theme: &Theme,
) -> AnyElement {
    let unreachable = matches!(reach, HostReach::Unreachable { .. });
    let tone = if unreachable {
        theme.palette.text.tertiary
    } else {
        theme.palette.text.secondary
    };
    let mut item = row_frame(id, focus, HOST_INSET, false, theme)
        .text_color(tone)
        .child(icon(IconName::Server, tone))
        .child(label(name.to_owned()));
    if let HostReach::Unreachable { minutes } = reach {
        item = item
            .child(
                div()
                    .flex_none()
                    .size(px(SIZE_INDICATOR_DOT))
                    .rounded(px(RADIUS_FULL))
                    .border_1()
                    .border_color(theme.palette.border.strong),
            )
            .child(
                div()
                    .flex_none()
                    .text_style(TYPE_CAPTION)
                    .text_color(theme.palette.text.tertiary)
                    .child(format!("{minutes} min ago")),
            );
    }
    item.into_any_element()
}

/// The frame every tree row shares: 28 px high, a tab stop with the focus ring, and the
/// Sidebar's solid fills. The selected row keeps its selected fill on hover.
fn row_frame(
    id: SharedString,
    focus: &FocusHandle,
    inset: f32,
    selected: bool,
    theme: &Theme,
) -> Stateful<Div> {
    let fills = Surface::Frame.fills(theme);
    let row = div()
        .id(id)
        .track_focus(focus)
        .tab_stop(true)
        .focus_ring(theme)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(SPACE_2))
        .h(px(SIZE_CONTROL_DEFAULT))
        .pl(px(inset))
        .pr(px(SPACE_2))
        .rounded(px(RADIUS_MD))
        .text_style(TYPE_BODY)
        .text_color(theme.palette.text.secondary);
    if selected {
        row.bg(fills.selected)
            .text_color(theme.palette.text.primary)
            .font_weight(FontWeight::SEMIBOLD)
    } else {
        row.hover(move |style| style.bg(fills.hover))
            .active(move |style| style.bg(fills.pressed))
    }
}

fn icon(name: IconName, color: Color) -> impl IntoElement {
    Icon::new(name).size(IconSize::Small).color(color)
}

/// The label takes the free width and truncates; its full text stays in the row's data.
fn label(text: impl Into<SharedString>) -> impl IntoElement {
    div().flex_1().min_w_0().truncate().child(text.into())
}

/// The icon of each navigation row. Two Lucide names are not bundled yet and use the nearest
/// bundled icon: Prototypes (`layers`) and Automations (`calendar-clock`).
fn nav_icon(kind: NavKind) -> IconName {
    match kind {
        NavKind::Prototypes => IconName::Palette,
        NavKind::Tasks => IconName::ListChecks,
        NavKind::Automations => IconName::Workflow,
        NavKind::Mobile => IconName::Smartphone,
        NavKind::ConferenceRoom => IconName::Users,
        NavKind::Board => IconName::Kanban,
    }
}

/// The status glyph of a session state, shared with the palette's session rows.
pub(super) fn glyph_state(state: AgentState) -> GlyphState {
    match state {
        AgentState::Running => GlyphState::Running,
        AgentState::Waiting => GlyphState::Waiting,
        AgentState::Idle => GlyphState::Idle,
        AgentState::Error => GlyphState::Error,
        AgentState::Unreachable => GlyphState::Unreachable,
        AgentState::Done => GlyphState::Done,
    }
}
