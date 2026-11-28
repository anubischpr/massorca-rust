//! The Sidebar: Create, Search, the navigation rows, the project tree and its footer (D-SH-01).
//! In Bot Teams it lists the teams instead (D-WS-29).

use std::collections::BTreeSet;

use super::Shell;
use super::session_rename::SessionRename;
use super::sidebar_rows::{nav_row, tree_item};
use super::teams;
use super::tooltip::with_tooltip;
use crate::model::morph::ease;
use crate::model::shell_state::ShellModel;
use crate::model::sidebar::{NavRow, SidebarContent, TreeRow, collapse_projects, session_indent};
use gpui::{
    Animation, AnimationExt, AnyElement, App, Context, Div, Entity, FocusHandle, IntoElement,
    MouseButton, SharedString, Stateful, Window, div, prelude::*, px,
};
use massorca_ui::components::button::{Button, ButtonSize, ButtonVariant};
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::components::search_field::SearchField;
use massorca_ui::theme::box_shadows;
use massorca_ui::tokens::{
    DURATION_FAST, EASE_STANDARD, LAYOUT_SIDEBAR_DEFAULT, RADIUS_LG, SIZE_CONTROL_DEFAULT, SPACE_1,
    SPACE_2, SPACE_HALF, TYPE_BODY_SM, TYPE_OVERLINE,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

/// What the Sidebar shows beyond its tree: the collapsed projects, the session title being edited, and the Show archived
/// toggle, drawn only while the service offers `agent-archive/1` (`archive` is its state then; decision 0163).
pub(super) struct SidebarView<'a> {
    pub(super) collapsed: &'a BTreeSet<String>,
    pub(super) renaming: Option<&'a SessionRename>,
    pub(super) archive: Option<bool>,
}

/// The Sidebar column. Inline it sits beside the Canvas; as the overlay it floats over the
/// Canvas when the window is too narrow for it (X1). `switch` is the mode switch in progress,
/// if any: its content fades in over it.
pub(super) fn sidebar(
    model: &ShellModel,
    view: SidebarView<'_>,
    search: &Entity<SearchField>,
    overlay: bool,
    switch: Option<u64>,
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
    let content = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap(px(SPACE_HALF));
    let content = match model.sidebar() {
        SidebarContent::Workspaces { nav, tree } => {
            let content = content.child(create_button(&shell)).child(search.clone());
            let content = nav_rows(content, &nav, "nav", &shell, window, cx);
            let archive = view.archive.map(|shown| archive_toggle(&shell, shown));
            content
                .child(projects_header(&theme, archive))
                .child(tree_list(
                    &collapse_projects(tree, view.collapsed),
                    &shell,
                    window,
                    cx,
                    &theme,
                    view.renaming,
                ))
                .child(footer())
        }
        SidebarContent::BotTeams => {
            let content = content.child(search.clone());
            teams::teams_list(content, &model.bot_teams_sidebar(), &shell, window, cx)
        }
    };
    column.child(fade_in(content, switch, cx))
}

/// The content of a Sidebar fades in over `duration.fast` when a mode switch starts (screens 3).
/// Reduced motion swaps it at once. The outgoing content is replaced, not faded out.
fn fade_in(content: Div, switch: Option<u64>, cx: &App) -> AnyElement {
    match switch {
        Some(generation) if !cx.reduce_motion() => content
            .with_animation(
                SharedString::from(format!("sidebar-fade-{generation}")),
                Animation::new(DURATION_FAST).with_easing(|t| ease(EASE_STANDARD, t)),
                |content, delta| content.opacity(delta),
            )
            .into_any_element(),
        _ => content.into_any_element(),
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
pub(super) fn row_focus(window: &mut Window, cx: &mut App, key: String) -> FocusHandle {
    window
        .use_keyed_state(SharedString::from(key), cx, |_, cx| cx.focus_handle())
        .read(cx)
        .clone()
}

fn create_button(shell: &gpui::WeakEntity<Shell>) -> impl IntoElement {
    let shell = shell.clone();
    Button::new("sidebar-create", "Create")
        .variant(ButtonVariant::Primary)
        .size(ButtonSize::Default)
        .leading_icon(IconName::Plus)
        .on_click(move |window, cx| {
            let _ = shell.update(cx, |shell, cx| shell.open_create_start(window, cx));
        })
}

pub(super) fn projects_header(theme: &Theme, archive: Option<AnyElement>) -> impl IntoElement {
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
                ))
                .when_some(archive, |buttons, toggle| buttons.child(toggle)),
        )
}

/// The Show archived toggle: pressed while archived sessions are listed (decision 0163).
fn archive_toggle(shell: &gpui::WeakEntity<Shell>, shown: bool) -> AnyElement {
    let shell = shell.clone();
    let icon = if shown {
        IconName::Eye
    } else {
        IconName::EyeOff
    };
    div()
        .debug_selector(|| "sidebar-show-archived".to_owned())
        .child(with_tooltip(
            IconButton::new("sidebar-show-archived", icon, "Show archived")
                .size(ButtonSize::Compact)
                .toggle(shown)
                .on_click(move |_, cx| {
                    let _ = shell.update(cx, |shell, cx| {
                        let show = !shell.model.show_archived();
                        shell.model.set_show_archived(show);
                        cx.notify();
                    });
                }),
            "Show archived",
        ))
        .into_any_element()
}

fn header_button(id: &'static str, icon: IconName, label: &'static str) -> impl IntoElement {
    with_tooltip(
        IconButton::new(id, icon, label).size(ButtonSize::Compact),
        label,
    )
}

/// A session row whose title is being edited: the field takes the row's place, and Escape closes it (A97).
fn rename_row(rename: &SessionRename, depth: usize, shell: &gpui::WeakEntity<Shell>) -> AnyElement {
    let (shell, selector) = (shell.clone(), format!("sidebar-rename-{}", rename.session));
    div()
        .debug_selector(move || selector.clone())
        .flex()
        .items_center()
        .pl(px(session_indent(depth)))
        .pr(px(SPACE_2))
        .on_key_down(move |event, _, cx| {
            if event.keystroke.key == "escape" {
                let _ = shell.update(cx, |shell, cx| shell.finish_session_rename(false, cx));
            }
        })
        .child(rename.field.clone())
        .into_any_element()
}

/// The project tree, scrollable on its own. Empty, it says so (A10). A session being renamed shows its field.
fn tree_list(
    tree: &[TreeRow],
    shell: &gpui::WeakEntity<Shell>,
    window: &mut Window,
    cx: &mut App,
    theme: &Theme,
    renaming: Option<&SessionRename>,
) -> Stateful<Div> {
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
    for (index, row) in tree.iter().enumerate() {
        if let (Some(rename), TreeRow::Session { id, depth, .. }) = (renaming, row)
            && rename.session == *id
        {
            list = list.child(rename_row(rename, *depth, shell));
            continue;
        }
        let focus = row_focus(window, cx, format!("tree-{index}"));
        list = list.child(tree_item(row, index, &focus, shell));
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

impl Shell {
    /// Collapses a project's rows, or expands them again (A10). The next start keeps the fold (SET-02).
    pub(super) fn toggle_project(&mut self, id: &str) {
        if !self.collapsed_projects.remove(id) {
            self.collapsed_projects.insert(id.to_owned());
        }
    }
}
