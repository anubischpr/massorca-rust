//! The Sidebar in Bot Teams mode (D-WS-29, screens 4.1): "All teams", every team with its
//! Conference room and Board, then the Projects of the team in view. Each row is a SidebarRow.

use super::super::Shell;
use super::super::change;
use super::super::sidebar::row_focus;
use super::filters::sidebar_filters_button;
use super::new_project::new_project_button;
use crate::model::navigation::Page;
use crate::model::shell::HostReach;
use crate::model::teams::{
    ChildRow, ProjectRow, ProjectRowKind, ProjectsSection, TeamId, TeamPage, TeamRow, TeamsSidebar,
};
use gpui::{App, Div, FocusHandle, WeakEntity, Window, div, prelude::*, px};
use massorca_ui::components::sidebar_row::{HostConnection, SidebarRow};
use massorca_ui::tokens::{
    SPACE_1, SPACE_2, SPACE_HALF, TYPE_BODY_SM, TYPE_CAPTION, TYPE_OVERLINE,
};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

/// Appends the Bot Teams rows to the Sidebar column, below its search field.
pub(in crate::shell) fn teams_list(
    column: Div,
    sidebar: &TeamsSidebar,
    shell: &WeakEntity<Shell>,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    let theme = *cx.theme();
    let mut column = column.child(all_teams_row(sidebar.all_teams_selected, shell, window, cx));
    if !sidebar.connected {
        return column.child(note("Bot Teams is not connected.", &theme));
    }
    if sidebar.teams.is_empty() {
        column = column.child(note("No teams yet", &theme));
    }
    for (index, team) in sidebar.teams.iter().enumerate() {
        column = column.child(team_rows(team, index, shell, window, cx));
    }
    match &sidebar.projects {
        Some(section) => column
            .child(teams_projects_header(section.team, shell, &theme))
            .child(projects_rows(section, shell, window, cx)),
        None => column,
    }
}

/// The PROJECTS header of the team in view: Filters and New project (4.1).
fn teams_projects_header(
    team: TeamId,
    shell: &WeakEntity<Shell>,
    theme: &Theme,
) -> impl IntoElement {
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
                .child(sidebar_filters_button(shell))
                .child(new_project_button(team, shell)),
        )
}

/// "All teams" is the mode's home: the Bot teams page with the team cards (D-AG-13).
fn all_teams_row(
    selected: bool,
    shell: &WeakEntity<Shell>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let focus = row_focus(window, cx, "teams-all".into());
    let shell = shell.clone();
    SidebarRow::nav("teams-all", IconName::LayoutGrid, "All teams")
        .focus_handle(focus)
        .tab_stop(true)
        .selected(selected)
        .on_click(move |_, _, cx| change(&shell, cx, |shell| shell.model.open_page(Page::Home)))
}

/// A team's row, then its Conference room and Board. A team whose host is unreachable dims only
/// itself and shows how long ago it was reached (A10).
fn team_rows(
    team: &TeamRow,
    index: usize,
    shell: &WeakEntity<Shell>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let theme = *cx.theme();
    let focus = row_focus(window, cx, format!("team-{index}"));
    let mut row = match team.reach {
        HostReach::Connected => SidebarRow::nav(
            format!("team-row-{index}"),
            IconName::Users,
            team.name.clone(),
        ),
        // An unreachable team reads like its host row: the status glyph, dimmed (A10).
        HostReach::Unreachable { .. } => SidebarRow::host(
            format!("team-row-{index}"),
            team.name.clone(),
            HostConnection::Unreachable,
        ),
    }
    .focus_handle(focus)
    .tab_stop(true)
    .selected(team.selected);
    if let HostReach::Unreachable { minutes } = team.reach {
        row = row
            .dimmed(true)
            .secondary(format!("Last known state, {minutes} min ago"));
    }
    let conference = TeamPage::Conference(team.id);
    let shell_for_row = shell.clone();
    let mut column = div().flex().flex_col().child(row.on_click(move |_, _, cx| {
        change(&shell_for_row, cx, move |shell| {
            shell.model.open_team_page(conference)
        });
    }));
    for (child, key) in [(&team.conference, "conference"), (&team.board, "board")] {
        let focus = row_focus(window, cx, format!("team-{index}-{key}"));
        column = column.child(child_row(child, index, key, focus, shell, &theme));
    }
    column
}

fn child_row(
    child: &ChildRow,
    index: usize,
    key: &str,
    focus: FocusHandle,
    shell: &WeakEntity<Shell>,
    theme: &massorca_ui::Theme,
) -> impl IntoElement {
    let icon = match child.page {
        TeamPage::Board(_) => IconName::Kanban,
        _ => IconName::MessagesSquare,
    };
    let page = child.page;
    let shell = shell.clone();
    let hint = div()
        .text_style(TYPE_CAPTION)
        .text_color(theme.palette.text.tertiary)
        .child(child.hint.clone());
    SidebarRow::nav(format!("team-{index}-{key}-row"), icon, child.label)
        .level(1)
        .focus_handle(focus)
        .tab_stop(true)
        .selected(child.selected)
        .trailing(hint)
        .on_click(move |_, _, cx| change(&shell, cx, move |shell| shell.model.open_team_page(page)))
}

/// The Projects section of the team in view: Files, Memory, Routines and its projects (4.1).
fn projects_rows(
    section: &ProjectsSection,
    shell: &WeakEntity<Shell>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let mut column = div().flex().flex_col().gap(px(SPACE_HALF));
    for (index, row) in section.rows.iter().enumerate() {
        let focus = row_focus(window, cx, format!("project-row-{index}"));
        column = column.child(project_row(row, index, focus, shell));
    }
    column
}

fn project_row(
    row: &ProjectRow,
    index: usize,
    focus: FocusHandle,
    shell: &WeakEntity<Shell>,
) -> impl IntoElement {
    let icon = match row.kind {
        ProjectRowKind::Files => IconName::Folder,
        ProjectRowKind::Memory => IconName::Brain,
        ProjectRowKind::Routines => IconName::CalendarClock,
        ProjectRowKind::Project(_) => IconName::FolderTree,
    };
    let page = row.page;
    let shell = shell.clone();
    SidebarRow::nav(format!("project-{index}"), icon, row.label.clone())
        .focus_handle(focus)
        .tab_stop(true)
        .selected(row.selected)
        .on_click(move |_, _, cx| change(&shell, cx, move |shell| shell.model.open_team_page(page)))
}

fn note(text: &'static str, theme: &massorca_ui::Theme) -> impl IntoElement {
    div()
        .px(px(SPACE_2))
        .py(px(SPACE_1))
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.tertiary)
        .child(text)
}
