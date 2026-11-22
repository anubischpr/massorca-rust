//! The Project sidebar (D-SH-04): its icon tabs and the body of the open tab. The Source control tab
//! is the panel of the selected workspace's changes, commit box and history (screen D-DF-02).

use gpui::{AnyElement, App, Div, IntoElement, SharedString, WeakEntity, div, prelude::*, px};
use massorca_ui::components::badge::{Badge, BadgeTone};
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::empty_state::EmptyState;
use massorca_ui::components::error_state::ErrorState;
use massorca_ui::components::project_sidebar_tabs::{ProjectSidebarTabs, core_tabs};
use massorca_ui::components::scroll_area::ScrollArea;
use massorca_ui::tokens::{SPACE_1, SPACE_2, SPACE_3, TYPE_BODY_STRONG, TYPE_CAPTION};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

use super::Shell;
use super::source_control::ProjectTab;
use crate::model::source_control::{Panel, PanelView, panel_view};

/// The Project sidebar's body under its header: the tab strip, then the open tab.
pub(super) fn project_body(shell: &Shell, handle: &WeakEntity<Shell>, cx: &App) -> AnyElement {
    let tab = shell.source_control.tab;
    let select = handle.clone();
    let tabs = ProjectSidebarTabs::new("project-tabs", core_tabs(changed_files(shell, cx), 0))
        .selected(tab.index())
        .on_select(move |index, _, cx| {
            if let Some(tab) = ProjectTab::at(index) {
                let _ = select.update(cx, |shell, cx| shell.select_project_tab(tab, cx));
            }
        });
    let content = match tab {
        ProjectTab::SourceControl => source_control(shell, handle, cx),
        other => EmptyState::new("project-tab-empty", other.label())
            .explanation("Not available yet.")
            .into_any_element(),
    };
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(tabs)
        .child(content)
        .into_any_element()
}

/// The number of changed files, for the Source control tab's count.
fn changed_files(shell: &Shell, cx: &App) -> u32 {
    let Some(workspace) = shell.model.selected_workspace() else {
        return 0;
    };
    match panel_view(shell.store.read(cx).state().git(workspace)) {
        PanelView::Ready(panel) => panel.changed_paths(),
        _ => 0,
    }
}

/// The Source control tab: the panel of the selected workspace, or the state it is in.
fn source_control(shell: &Shell, handle: &WeakEntity<Shell>, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let Some(workspace) = shell.model.selected_workspace() else {
        return note(
            "No workspace selected",
            "Open a workspace to see its changes.",
            &theme,
        );
    };
    let body = match panel_view(shell.store.read(cx).state().git(workspace)) {
        PanelView::Loading => note("Reading the changes", "", &theme),
        PanelView::NotRepository => EmptyState::new(
            "source-control-folder",
            "This folder is not a Git repository.",
        )
        .icon(IconName::GitBranch)
        .explanation("Source control needs Git. You can keep working in the folder as it is.")
        .into_any_element(),
        PanelView::Failed { message } => {
            ErrorState::new("source-control-failed", "Could not read the changes.")
                .explanation(message)
                .on_retry(retry(handle))
                .into_any_element()
        }
        PanelView::Ready(panel) => panel_body(shell, handle, &panel, &theme),
    };
    ScrollArea::new("source-control-scroll")
        .child(body)
        .into_any_element()
}

/// Asks the service again for a panel that could not read its changes.
fn retry(handle: &WeakEntity<Shell>) -> impl Fn(&mut gpui::Window, &mut App) + 'static {
    let handle = handle.clone();
    move |_, cx| {
        let _ = handle.update(cx, |shell, cx| shell.retry_source_control(cx));
    }
}

/// The panel of a workspace that has its status: the branch, notices, the commit box, the changes
/// in their groups and the history.
fn panel_body(
    shell: &Shell,
    handle: &WeakEntity<Shell>,
    panel: &Panel,
    theme: &Theme,
) -> AnyElement {
    let mut column = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .p(px(SPACE_3))
        .child(branch_line(panel, theme));
    if let Some(operation) = panel.operation {
        column = column.child(
            Banner::new(
                "source-control-operation",
                BannerVariant::Warning,
                operation,
            )
            .detail("Resolve it in your terminal, then come back."),
        );
    }
    if let Some(reason) = &panel.degraded {
        column = column.child(
            Banner::new(
                "source-control-degraded",
                BannerVariant::Warning,
                "Live updates are off for this workspace.",
            )
            .detail(reason.clone()),
        );
    }
    if panel.truncated {
        column = column.child(Banner::new(
            "source-control-truncated",
            BannerVariant::Info,
            "Only the first changes are listed.",
        ));
    }
    column = column.child(commit_box(shell, handle, panel, theme));
    if panel.is_clean() {
        column = column.child(note("Nothing to commit.", "", theme));
    }
    column.into_any_element()
}

/// The branch with its upstream, its ahead and behind counts, and the states that need a word.
fn branch_line(panel: &Panel, theme: &Theme) -> AnyElement {
    let branch = &panel.branch;
    let mut line = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(SPACE_2))
        .child(
            div()
                .text_style(TYPE_BODY_STRONG)
                .text_color(theme.palette.text.primary)
                .child(branch.name.clone()),
        );
    if branch.detached {
        line = line.child(Badge::text("Detached HEAD").tone(BadgeTone::Warning));
    }
    if let Some(upstream) = &branch.upstream {
        line = line.child(caption(format!("→ {upstream}"), theme));
    }
    if branch.ahead > 0 || branch.behind > 0 {
        line = line.child(caption(
            format!("↑{} ↓{}", branch.ahead, branch.behind),
            theme,
        ));
    }
    if branch.upstream_gone {
        line = line.child(caption("Upstream is gone", theme));
    }
    if branch.unborn {
        line = line.child(caption("No commits yet", theme));
    }
    line.into_any_element()
}

/// The message field, the refusals the service gave, and the Commit button (A49).
fn commit_box(
    shell: &Shell,
    handle: &WeakEntity<Shell>,
    panel: &Panel,
    theme: &Theme,
) -> AnyElement {
    let form = &shell.source_control.commit;
    let submit = handle.clone();
    let label = if panel.branch.detached {
        "Commit anyway"
    } else {
        "Commit"
    };
    let mut column = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_1))
        .child(shell.source_control.commit_field.clone());
    if let Some(message) = &form.message_error {
        column = column.child(
            div()
                .text_style(TYPE_CAPTION)
                .text_color(theme.palette.status.danger_fg)
                .child(message.clone()),
        );
    }
    if let Some(error) = &form.error {
        column = column.child(Banner::new(
            "source-control-commit-error",
            BannerVariant::Danger,
            error.clone(),
        ));
    }
    column
        .child(
            div().flex().justify_end().child(
                Button::new("source-control-commit", label)
                    .variant(ButtonVariant::Primary)
                    .disabled(form.busy())
                    .on_click(move |_, cx| {
                        let _ = submit.update(cx, |shell, cx| shell.submit_commit(cx));
                    }),
            ),
        )
        .into_any_element()
}

/// A title and a line of explanation, for a state the panel is in.
fn note(title: &str, text: &str, theme: &Theme) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_1))
        .p(px(SPACE_3))
        .child(
            div()
                .text_style(TYPE_BODY_STRONG)
                .text_color(theme.palette.text.primary)
                .child(title.to_owned()),
        )
        .child(caption(text.to_owned(), theme))
        .into_any_element()
}

fn caption(text: impl Into<SharedString>, theme: &Theme) -> Div {
    div()
        .text_style(TYPE_CAPTION)
        .text_color(theme.palette.text.tertiary)
        .child(text.into())
}
