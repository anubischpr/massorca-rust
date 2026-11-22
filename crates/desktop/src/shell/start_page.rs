//! The workspace start page (D-AG-01, J3): what the Canvas shows for a workspace with no session
//! open. It names the workspace's project and host, the facts the service reported, and the actions
//! that cannot run yet, drawn disabled with the reason. Opening a project from the heading or the
//! placement row uses the project menu of the create form; choosing a row opens that project's
//! workspace or the start page with no workspace.

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, SharedString, WeakEntity, Window, div, px};
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::kbd::Kbd;
use massorca_ui::components::popover::OverlayLayer;
use massorca_ui::components::rose_logo::rose_logo;
use massorca_ui::components::segmented_control::{Segment, SegmentedControl};
use massorca_ui::components::settings_row::{SettingsRow, SettingsSection};
use massorca_ui::tokens::{
    LAYOUT_CONVERSATION_READING, RADIUS_LG, SPACE_1, SPACE_2, SPACE_3, SPACE_4, SPACE_6, SPACE_8,
    TYPE_CAPTION, TYPE_HEADING,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

use super::Shell;
use super::workspace_dialogs::{MenuTarget, project_menu};
use crate::model::start_page::{
    COMPOSER_PLACEHOLDER, GIT_LATER, KEY_HINTS, SESSIONS_LATER, WorkspaceStart,
};
use crate::model::work_panel::terminal_unavailable;
use crate::model::workspace_flow::ProjectChoice;

/// What the start page draws: the workspace, the shell to act through, and whether the service
/// is connected, which "Open terminal" needs.
pub(super) struct StartFrame {
    pub page: WorkspaceStart,
    pub shell: WeakEntity<Shell>,
    pub connected: bool,
}

/// The start page for one workspace, centred in the Canvas.
pub(super) fn start_page(frame: StartFrame, cx: &mut App) -> AnyElement {
    let theme = *cx.theme();
    let p = &theme.palette;
    let page = &frame.page;

    let project_link = {
        let shell = frame.shell.clone();
        div()
            .id("start-project-link")
            .debug_selector(|| "start-project-link".to_owned())
            .cursor_pointer()
            .underline()
            .child(page.project.clone())
            .on_click(move |_, window, cx| {
                let _ = shell.update(cx, |shell, cx| shell.open_start_project_menu(window, cx));
            })
    };
    let heading = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .justify_center()
        .text_style(TYPE_HEADING)
        .text_color(p.text.primary)
        .child("What should we work on in ")
        .child(project_link);

    let placement = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .justify_center()
        .gap(px(SPACE_2))
        .child(
            Button::new("start-host", page.host)
                .variant(ButtonVariant::Ghost)
                .leading_icon(IconName::Server)
                .disabled(true)
                .tooltip("Hosts other than This PC come later."),
        )
        .child(
            Button::new("start-project", page.project.clone())
                .variant(ButtonVariant::Ghost)
                .trailing_icon(IconName::ChevronDown)
                .disabled(true)
                .tooltip("Change the project from the heading."),
        )
        .child(
            Button::new(
                "start-branch",
                page.branch
                    .clone()
                    .unwrap_or_else(|| "No branch".to_owned()),
            )
            .variant(ButtonVariant::Ghost)
            .leading_icon(IconName::GitBranch)
            .disabled(true)
            .tooltip(GIT_LATER),
        );

    // The Chat / Terminal switch sits in the composer's bottom row, never in the placement row (A32).
    let composer_box = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .p(px(SPACE_4))
        .rounded(px(RADIUS_LG))
        .border_1()
        .border_color(p.border.default)
        .bg(p.surface.raised)
        .child(
            div()
                .text_body()
                .text_color(p.text.tertiary)
                .child(COMPOSER_PLACEHOLDER),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    SegmentedControl::new(
                        "start-mode",
                        "start-mode-group",
                        vec![
                            Segment::icon_text(IconName::MessageSquare, "Chat"),
                            Segment::icon_text(IconName::SquareTerminal, "Terminal"),
                        ],
                    )
                    .disabled(true),
                )
                .child(
                    Button::new("start-send", "Send")
                        .variant(ButtonVariant::Primary)
                        .disabled(true)
                        .tooltip(SESSIONS_LATER),
                ),
        );
    let composer = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .child(composer_box)
        .child(key_hints(&theme));

    let facts = SettingsSection::new("start-facts", "Workspace")
        .row(fact_row("start-name", "Name", page.name.clone()))
        .row(fact_row(
            "start-project-fact",
            "Project",
            page.project.clone(),
        ))
        .row(fact_row("start-path", "Path", page.path.clone()))
        .row(fact_row(
            "start-branch-fact",
            "Branch",
            page.branch.clone().unwrap_or_else(|| "None".to_owned()),
        ))
        .row(fact_row("start-state", "State", page.state.to_owned()));

    let mut column = div()
        .w_full()
        .max_w(px(LAYOUT_CONVERSATION_READING))
        .flex()
        .flex_col()
        .gap(px(SPACE_6))
        .child(div().flex().justify_center().child(rose_logo()))
        .child(heading)
        .child(placement)
        .child(composer);
    if page.missing {
        column = column.child(
            Banner::new(
                "start-missing",
                BannerVariant::Warning,
                "This folder is missing from disk.",
            )
            .detail(page.path.clone()),
        );
    }
    column = column.child(facts).child(actions(&frame));

    div()
        .id("start-page")
        .size_full()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .items_center()
        .p(px(SPACE_8))
        .child(column)
        .into_any_element()
}

/// The key hints under the composer: each key in a keycap, then what it does (D-AG-01).
fn key_hints(theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .justify_center()
        .gap(px(SPACE_4))
        .children(KEY_HINTS.iter().map(|(key, what)| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(SPACE_1))
                .text_style(TYPE_CAPTION)
                .text_color(theme.palette.text.tertiary)
                .child(Kbd::new(*key))
                .child(*what)
        }))
}

/// One read-only fact of the workspace, as a settings row with its value on the right.
fn fact_row(id: &'static str, label: &'static str, value: String) -> SettingsRow {
    SettingsRow::new(id, label).control(div().text_body().child(SharedString::from(value)))
}

/// "Open terminal": shows the Work panel with a new terminal in this workspace. Off, with its
/// reason, while the service is not connected.
fn open_terminal_button(frame: &StartFrame) -> impl IntoElement {
    let (shell, workspace) = (frame.shell.clone(), frame.page.id.clone());
    let button = Button::new("start-terminal", "Open terminal")
        .variant(ButtonVariant::Secondary)
        .leading_icon(IconName::SquareTerminal);
    let button = match terminal_unavailable(true, frame.connected) {
        Some(reason) => button.disabled(true).tooltip(reason),
        None => button.on_click(move |window, cx| {
            let _ = shell.update(cx, |shell, cx| {
                shell.open_terminal(workspace.clone(), window, cx);
            });
        }),
    };
    div()
        .debug_selector(|| "start-terminal".to_owned())
        .child(button)
}

/// The actions the page will have. Removing the workspace uses the existing flow; the rest wait
/// for the layer they need.
fn actions(frame: &StartFrame) -> AnyElement {
    let shell = frame.shell.clone();
    let id = frame.page.id.clone();
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(px(SPACE_2))
        .child(open_terminal_button(frame))
        .child(
            Button::new("start-source-control", "Source control")
                .variant(ButtonVariant::Secondary)
                .leading_icon(IconName::GitBranch)
                .disabled(true)
                .tooltip(GIT_LATER),
        )
        .child(
            Button::new("start-remove", "Delete workspace…")
                .variant(ButtonVariant::Ghost)
                .leading_icon(IconName::Trash2)
                .on_click(move |_, cx| {
                    let _ = shell.update(cx, |shell, cx| shell.open_remove(&id, cx));
                }),
        )
        .into_any_element()
}

impl Shell {
    /// The project menu of the start page: the rows the create form uses, with this workspace's
    /// project checked. It opens where the pointer is.
    pub(super) fn open_start_project_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.store.read(cx).state().clone();
        let current = self
            .model
            .selected_workspace()
            .and_then(|id| crate::model::start_page::workspace_start(&state, id))
            .map(|page| ProjectChoice::Project(page.project_id));
        let (entries, choices) = project_menu(&state.projects, current.as_ref());
        self.open_menu(
            entries,
            MenuTarget::StartProject(choices),
            OverlayLayer::Dialog,
            window.mouse_position(),
            window,
            cx,
        );
    }

    /// Choosing a project opens its first workspace; "No project" opens the start page with no
    /// workspace. Nothing is created.
    pub(super) fn open_start_project(&mut self, choice: ProjectChoice, cx: &mut Context<Self>) {
        let workspace = match choice {
            ProjectChoice::Project(id) => {
                crate::model::start_page::first_workspace_of(self.store.read(cx).state(), &id)
            }
            ProjectChoice::NoProject => None,
        };
        self.model.select_workspace(workspace);
        cx.notify();
    }
}
