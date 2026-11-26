//! The Project sidebar (D-SH-04): its icon tabs and the body of the open tab. The Source control tab
//! is the panel of the selected workspace's changes, commit box and history (screen D-DF-02).

use gpui::{AnyElement, App, Div, IntoElement, SharedString, WeakEntity, div, prelude::*, px};
use massorca_protocol::methods::git::diff::DiffTarget;
use massorca_ui::components::badge::{Badge, BadgeTone};
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::components::button::{Button, ButtonSize, ButtonVariant};
use massorca_ui::components::empty_state::EmptyState;
use massorca_ui::components::error_state::ErrorState;
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::components::list_row::{ListRow, RowLeading};
use massorca_ui::components::project_sidebar_tabs::{ProjectSidebarTabs, core_tabs};
use massorca_ui::components::scroll_area::ScrollArea;
use massorca_ui::tokens::{
    SPACE_1, SPACE_2, SPACE_3, TYPE_BODY_SM, TYPE_BODY_STRONG, TYPE_CAPTION, TYPE_MONO_SM,
    TYPE_OVERLINE,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

use super::Shell;
use super::source_control::{OpenFile, ProjectTab};
use crate::model::source_control::{ChangeRow, CommitRow, History, Panel, PanelView, panel_view};
use crate::model::source_control_flow::DiscardTarget;
use crate::model::source_control_sync::{SyncAction, SyncTone};

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
    let remove = remove_project_control(shell, handle, cx);
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(tabs)
        .child(content)
        .children(remove)
        .into_any_element()
}

/// The Remove project button under the tabs, for the project of the selected workspace. It is absent
/// without `projects/1` and while no workspace is selected.
fn remove_project_control(
    shell: &Shell,
    handle: &WeakEntity<Shell>,
    cx: &App,
) -> Option<AnyElement> {
    let state = shell.store.read(cx).state();
    if !state.project_edits {
        return None;
    }
    let project = state
        .workspace(shell.model.selected_workspace()?)?
        .project_id
        .clone();
    let handle = handle.clone();
    Some(
        div()
            .flex()
            .justify_end()
            .p(px(SPACE_2))
            .child(
                Button::new("project-remove", "Remove project")
                    .variant(ButtonVariant::DangerGhost)
                    .size(ButtonSize::Compact)
                    .on_click(move |_, cx| {
                        let _ =
                            handle.update(cx, |shell, cx| shell.open_project_remove(&project, cx));
                    }),
            )
            .into_any_element(),
    )
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
    let sync_offered = shell.store.read(cx).state().git_sync;
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
        PanelView::Ready(panel) => panel_body(shell, handle, &panel, &theme, sync_offered),
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

/// What the panel's rows need from the shell: who to ask, whether a call is out, which file is open.
struct Rows<'a> {
    handle: &'a WeakEntity<Shell>,
    busy: bool,
    open: Option<&'a str>,
    theme: &'a Theme,
}

/// The panel of a workspace that has its status: the branch, notices, the commit box, the changes
/// in their groups and the history.
fn panel_body(
    shell: &Shell,
    handle: &WeakEntity<Shell>,
    panel: &Panel,
    theme: &Theme,
    sync_offered: bool,
) -> AnyElement {
    let busy = shell.source_control.busy;
    let mut column = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .p(px(SPACE_3))
        .child(branch_line(panel, theme));
    if sync_offered {
        column = column.child(sync_row(handle, busy));
    }
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
    if let Some(error) = &shell.source_control.error {
        column = column.child(Banner::new(
            "source-control-error",
            BannerVariant::Danger,
            error.clone(),
        ));
    }
    if let Some(note) = &shell.source_control.notice {
        let variant = match note.tone {
            SyncTone::Info => BannerVariant::Info,
            SyncTone::Warning => BannerVariant::Warning,
        };
        column = column.child(Banner::new(
            "source-control-notice",
            variant,
            note.text.clone(),
        ));
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
    let rows = Rows {
        handle,
        busy,
        open: shell
            .source_control
            .open
            .as_ref()
            .map(|open| open.path.as_str()),
        theme,
    };
    let groups = [
        group(
            "Conflicted",
            &panel.conflicted,
            Group::Conflicted,
            None,
            &rows,
        ),
        group(
            "Staged changes",
            &panel.staged,
            Group::Staged,
            Some(bulk_button(&rows, "Unstage all", true)),
            &rows,
        ),
        group(
            "Changes",
            &panel.changes,
            Group::Changed,
            Some(bulk_button(&rows, "Stage all", false)),
            &rows,
        ),
        group("Untracked", &panel.untracked, Group::Untracked, None, &rows),
    ];
    column
        .children(groups.into_iter().flatten())
        .child(history(&panel.history, theme))
        .into_any_element()
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

/// Fetch, Pull and Push for the workspace the panel shows. Each waits while a call is out.
fn sync_row(handle: &WeakEntity<Shell>, busy: bool) -> AnyElement {
    let mut row = div().flex().flex_wrap().items_center().gap(px(SPACE_1));
    for action in SyncAction::ALL {
        let handle = handle.clone();
        row = row.child(
            Button::new(action.button_id(), action.label())
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Compact)
                .disabled(busy)
                .on_click(move |_, cx| {
                    let _ = handle.update(cx, |shell, cx| shell.run_sync(action, cx));
                }),
        );
    }
    row.into_any_element()
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

/// The groups the changes are listed in. Each has its own actions (stage, unstage, discard, or none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Group {
    Conflicted,
    Staged,
    Changed,
    Untracked,
}

/// One group with its heading, its count, an optional bulk action and its rows. None when empty.
fn group(
    title: &str,
    rows: &[ChangeRow],
    kind: Group,
    action: Option<AnyElement>,
    ctx: &Rows,
) -> Option<AnyElement> {
    if rows.is_empty() {
        return None;
    }
    let heading = div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_style(TYPE_OVERLINE)
                .text_color(ctx.theme.palette.text.tertiary)
                .child(format!("{title} · {}", rows.len())),
        )
        .children(action);
    let items: Vec<AnyElement> = rows.iter().map(|row| change_row(row, kind, ctx)).collect();
    Some(
        div()
            .flex()
            .flex_col()
            .gap(px(SPACE_1))
            .child(heading)
            .children(items)
            .into_any_element(),
    )
}

/// One changed file: its name and folder, a click that opens its diff, its line counts and status
/// letter, and its actions.
fn change_row(row: &ChangeRow, kind: Group, ctx: &Rows) -> AnyElement {
    let key = format!("source-control-{kind:?}-{}", row.path);
    let mut item = ListRow::new(SharedString::from(key), row.name.clone())
        .leading(RowLeading::Icon(IconName::FileText))
        .tooltip(row.path.clone())
        .selected(ctx.open == Some(row.path.as_str()));
    if !row.directory.is_empty() {
        item = item.secondary(row.directory.clone());
    }
    if let Some(file) = open_file_of(row, kind) {
        let handle = ctx.handle.clone();
        item = item.on_click(move |_, _, cx| {
            let _ = handle.update(cx, |shell, cx| shell.open_file(file.clone(), cx));
        });
    }
    let trailing = div()
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .children(counts(row, ctx.theme))
        .child(
            div()
                .text_style(TYPE_MONO_SM)
                .text_color(ctx.theme.palette.text.secondary)
                .child(row.letter),
        )
        .children(row_actions(row, kind, ctx));
    item.trailing(trailing).into_any_element()
}

/// The diff a row opens: the staged side for the staged group, the working tree otherwise. A conflict
/// opens nothing; it is resolved in the editor.
fn open_file_of(row: &ChangeRow, kind: Group) -> Option<OpenFile> {
    let target = match kind {
        Group::Staged => DiffTarget::Staged,
        Group::Changed | Group::Untracked => DiffTarget::Worktree,
        Group::Conflicted => return None,
    };
    Some(OpenFile {
        path: row.path.clone(),
        target,
        original_path: row.original_path.clone(),
        kind: row.kind,
    })
}

/// "+3 -1" for a text change, "binary" for a binary one, nothing where git counts no lines.
fn counts(row: &ChangeRow, theme: &Theme) -> Option<Div> {
    if row.binary {
        return Some(
            div()
                .text_style(TYPE_MONO_SM)
                .text_color(theme.palette.text.tertiary)
                .child("binary"),
        );
    }
    let (added, removed) = row.lines?;
    Some(
        div()
            .flex()
            .gap(px(SPACE_1))
            .text_style(TYPE_MONO_SM)
            .child(
                div()
                    .text_color(theme.palette.diff.add_fg)
                    .child(format!("+{added}")),
            )
            .child(
                div()
                    .text_color(theme.palette.diff.remove_fg)
                    .child(format!("-{removed}")),
            ),
    )
}

/// The row's own buttons: unstage in the staged group; stage and discard in the changed and untracked
/// groups; none for a conflict.
fn row_actions(row: &ChangeRow, kind: Group, ctx: &Rows) -> Vec<AnyElement> {
    match kind {
        Group::Conflicted => Vec::new(),
        Group::Staged => vec![stage_button(row, true, ctx)],
        Group::Changed | Group::Untracked => {
            vec![
                stage_button(row, false, ctx),
                discard_button(row, kind == Group::Untracked, ctx),
            ]
        }
    }
}

fn stage_button(row: &ChangeRow, staged: bool, ctx: &Rows) -> AnyElement {
    let (icon, name) = if staged {
        (IconName::Minus, format!("Unstage {}", row.name))
    } else {
        (IconName::Plus, format!("Stage {}", row.name))
    };
    let path = row.path.clone();
    let handle = ctx.handle.clone();
    IconButton::new(
        SharedString::from(format!("source-control-stage-{}", row.path)),
        icon,
        name,
    )
    .disabled(ctx.busy)
    .on_click(move |_, cx| {
        let _ = handle.update(cx, |shell, cx| {
            shell.set_staged(Some(path.clone()), staged, cx);
        });
    })
    .into_any_element()
}

/// Asks before discarding one file's changes (decision 0013).
fn discard_button(row: &ChangeRow, untracked: bool, ctx: &Rows) -> AnyElement {
    let path = row.path.clone();
    let handle = ctx.handle.clone();
    IconButton::new(
        SharedString::from(format!("source-control-discard-{}", row.path)),
        IconName::RotateCcw,
        format!("Discard changes in {}", row.name),
    )
    .disabled(ctx.busy)
    .on_click(move |_, cx| {
        let target = DiscardTarget {
            path: path.clone(),
            untracked,
        };
        let _ = handle.update(cx, |shell, cx| shell.open_discard(vec![target.clone()], cx));
    })
    .into_any_element()
}

/// "Stage all" or "Unstage all" for a whole group.
fn bulk_button(ctx: &Rows, label: &'static str, staged: bool) -> AnyElement {
    let handle = ctx.handle.clone();
    Button::new(label, label)
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::Compact)
        .disabled(ctx.busy)
        .on_click(move |_, cx| {
            let _ = handle.update(cx, |shell, cx| shell.set_staged(None, staged, cx));
        })
        .into_any_element()
}

/// The first page of the branch's commits, newest first.
fn history(history: &History, theme: &Theme) -> AnyElement {
    let mut column = div().flex().flex_col().gap(px(SPACE_1)).child(
        div()
            .text_style(TYPE_OVERLINE)
            .text_color(theme.palette.text.tertiary)
            .child("Commits in branch"),
    );
    match history {
        History::Loading => column = column.child(caption("Reading the history", theme)),
        History::Failed { message } => {
            column = column.child(caption(message.clone(), theme));
        }
        History::Loaded { commits, more } => {
            if commits.is_empty() {
                column = column.child(caption("No commits yet.", theme));
            }
            for commit in commits {
                column = column.child(commit_line(commit, theme));
            }
            if *more {
                column = column.child(caption("Showing the latest commits.", theme));
            }
        }
    }
    column.into_any_element()
}

fn commit_line(commit: &CommitRow, theme: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .min_w_0()
        .child(
            div()
                .flex_none()
                .text_style(TYPE_MONO_SM)
                .text_color(theme.palette.text.tertiary)
                .child(commit.short_id.clone()),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.primary)
                .child(commit.subject.clone()),
        )
        .child(
            div()
                .flex_none()
                .text_style(TYPE_CAPTION)
                .text_color(theme.palette.text.tertiary)
                .child(commit.author.clone()),
        )
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
