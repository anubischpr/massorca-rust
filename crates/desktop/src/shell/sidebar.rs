//! The Sidebar: Create, Search, the navigation rows, the project tree and its footer (D-SH-01).
//! In Bot Teams it lists the team placeholders instead.

use super::Shell;
use super::sidebar_rows::{nav_row, tree_row};
use super::tooltip::with_tooltip;
use crate::model::shell_state::ShellModel;
use crate::model::sidebar::{NavRow, SidebarContent, TreeRow};
use gpui::{
    App, Context, Div, Entity, FocusHandle, IntoElement, MouseButton, SharedString, Stateful,
    Window, div, prelude::*, px,
};
use massorca_ui::components::button::{Button, ButtonSize, ButtonVariant};
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::components::search_field::SearchField;
use massorca_ui::theme::box_shadows;
use massorca_ui::tokens::{
    LAYOUT_SIDEBAR_DEFAULT, RADIUS_LG, SIZE_CONTROL_DEFAULT, SPACE_1, SPACE_2, SPACE_HALF,
    TYPE_BODY_SM, TYPE_OVERLINE,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

/// The Sidebar column. Inline it sits beside the Canvas; as the overlay it floats over the
/// Canvas when the window is too narrow for it (X1).
pub(super) fn sidebar(
    model: &ShellModel,
    search: &Entity<SearchField>,
    overlay: bool,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Div {
    let theme = *cx.theme();
    let shell = cx.entity().downgrade();
    let column = div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(px(SPACE_HALF))
        .w(px(LAYOUT_SIDEBAR_DEFAULT))
        .h_full()
        .pt(px(SPACE_1))
        .px(px(SPACE_2))
        .pb(px(SPACE_2));
    let column = if overlay {
        column
            .absolute()
            .top_0()
            .left_0()
            .bottom_0()
            .bg(theme.palette.surface.overlay)
            .border_1()
            .border_color(theme.palette.border.default)
            .rounded_r(px(RADIUS_LG))
            .shadow(box_shadows(theme.shadows.e2))
            // A click on the overlay itself must not close it (the Canvas click does).
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
    } else {
        column.bg(theme.palette.surface.frame)
    };
    match model.sidebar() {
        SidebarContent::Workspaces { nav, tree } => {
            let column = column.child(create_button()).child(search.clone());
            let column = nav_rows(column, &nav, "nav", &shell, window, cx);
            column
                .child(projects_header(&theme))
                .child(tree_list(&tree, window, cx, &theme))
                .child(footer())
        }
        SidebarContent::BotTeams { rows } => nav_rows(column, &rows, "team", &shell, window, cx),
    }
}

fn nav_rows(
    column: Div,
    rows: &[NavRow],
    key: &str,
    shell: &gpui::WeakEntity<Shell>,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let mut column = column;
    for (index, row) in rows.iter().enumerate() {
        let focus = row_focus(window, cx, format!("{key}-{index}"));
        column = column.child(nav_row(row, &focus, shell));
    }
    column
}

/// The focus handle of the row with this key, kept across frames the way IconButton keeps its
/// own, so Tab and the arrow keys reach every row.
fn row_focus(window: &mut Window, cx: &mut App, key: String) -> FocusHandle {
    window
        .use_keyed_state(SharedString::from(key), cx, |_, cx| cx.focus_handle())
        .read(cx)
        .clone()
}

fn create_button() -> impl IntoElement {
    Button::new("sidebar-create", "Create")
        .variant(ButtonVariant::Primary)
        .size(ButtonSize::Default)
        .leading_icon(IconName::Plus)
}

fn projects_header(theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .pt(px(SPACE_2 + SPACE_HALF))
        .pb(px(SPACE_1))
        .px(px(SPACE_2))
        .child(
            div()
                .text_style(TYPE_OVERLINE)
                .text_color(theme.palette.text.secondary)
                .child("PROJECTS"),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(SPACE_HALF))
                .child(header_button(
                    "sidebar-notifications",
                    IconName::Bell,
                    "Notifications",
                ))
                .child(header_button(
                    "sidebar-filter",
                    IconName::ListFilter,
                    "Filter and sort",
                ))
                .child(header_button(
                    "sidebar-add-folder",
                    IconName::FolderPlus,
                    "Add folder",
                ))
                .child(header_button(
                    "sidebar-new-project",
                    IconName::Plus,
                    "New project",
                )),
        )
}

fn header_button(id: &'static str, icon: IconName, label: &'static str) -> impl IntoElement {
    with_tooltip(
        IconButton::new(id, icon, label).size(ButtonSize::Compact),
        label,
    )
}

/// The project tree, scrollable on its own. Empty, it says so (A10).
fn tree_list(tree: &[TreeRow], window: &mut Window, cx: &mut App, theme: &Theme) -> Stateful<Div> {
    let mut list = div()
        .id("sidebar-tree")
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .gap(px(SPACE_HALF))
        .overflow_y_scroll();
    if tree.is_empty() {
        list = list.child(
            div()
                .px(px(SPACE_2))
                .py(px(SPACE_1))
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.tertiary)
                .child("No projects yet"),
        );
    }
    let mut workspaces = 0;
    for (index, row) in tree.iter().enumerate() {
        let focus = row_focus(window, cx, format!("tree-{index}"));
        list = list.child(tree_row(row, index, workspaces, &focus));
        if matches!(row, TreeRow::Workspace { .. }) {
            workspaces += 1;
        }
    }
    list
}

/// The footer row: Help, Locate and the panel toggle. They are placeholders until their
/// features exist.
fn footer() -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(SPACE_HALF))
        .h(px(SIZE_CONTROL_DEFAULT))
        .child(footer_button("sidebar-help", IconName::CircleHelp, "Help"))
        .child(footer_button("sidebar-locate", IconName::Locate, "Locate"))
        .child(footer_button(
            "sidebar-toggle-panel",
            IconName::PanelLeft,
            "Toggle panel",
        ))
}

fn footer_button(id: &'static str, icon: IconName, label: &'static str) -> impl IntoElement {
    with_tooltip(
        IconButton::new(id, icon, label).size(ButtonSize::Default),
        label,
    )
}
