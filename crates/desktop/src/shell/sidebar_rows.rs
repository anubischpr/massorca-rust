//! Rows of the Sidebar: maps the navigation rows and the project tree of the view model onto the
//! shared SidebarRow (A10, A83 to A85, A84, A97, P29).

use super::Shell;
use super::change;
use super::project_mark::decoded_image;
use super::rail::agent_kind;
use crate::model::project_icon::ProjectGlyph;
use crate::model::shell::{Agent, AgentState, HostReach};
use crate::model::sidebar::{HOST_INDENT, NavKind, NavRow, TreeRow, session_indent};
use crate::model::workspace_flow::NO_PROJECT_LABEL;
use gpui::{AnyElement, FocusHandle, SharedString, WeakEntity, div, prelude::*, px};
use massorca_ui::IconName;
use massorca_ui::components::agent_status::AgentState as GlyphState;
use massorca_ui::components::badge::{Badge, BadgeTone};
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::components::list_row::TREE_INDENT;
use massorca_ui::components::project_mark::{emoji_mark, image_mark, lucide_icon};
use massorca_ui::components::sidebar_row::{HostConnection, SidebarRow};
use massorca_ui::tokens::SPACE_2;

/// The tree level of each kind, for assistive tech: host 1, session 2, child sessions one deeper.
const HOST_LEVEL: usize = 1;
const SESSION_LEVEL: usize = 2;

/// A navigation row: an icon, its label and an optional count. A row with a page opens it.
pub(super) fn nav_row(row: &NavRow, focus: &FocusHandle, shell: &WeakEntity<Shell>) -> SidebarRow {
    let mut item = SidebarRow::nav(row.label, nav_icon(row.kind), row.label)
        .focus_handle(focus.clone())
        .tab_stop(true);
    if let Some(count) = row.count {
        item = item.count(count);
    }
    if row.chevron {
        item = item.expanded(false);
    }
    if let Some(page) = row.page {
        let shell = shell.clone();
        item = item.on_click(move |_, _, cx| {
            change(&shell, cx, move |shell| shell.model.open_page(page));
        });
    }
    item
}

/// A project's row: the struck folder for the "No project" group (A95), else the icon its owner chose (decision 0091).
fn project_row(id: SharedString, name: &str, icon: &ProjectGlyph) -> SidebarRow {
    if name == NO_PROJECT_LABEL {
        return SidebarRow::project(id, IconName::FolderX, name);
    }
    match icon {
        ProjectGlyph::Folder => SidebarRow::project(id, IconName::Folder, name),
        ProjectGlyph::Lucide(icon_name) => {
            SidebarRow::project(id, lucide_icon(icon_name).unwrap_or(IconName::Folder), name)
        }
        ProjectGlyph::Emoji(emoji) => {
            SidebarRow::project_element(id, emoji_mark(emoji.clone().into()), name)
        }
        ProjectGlyph::Image(image) => {
            SidebarRow::project_element(id, image_mark(decoded_image(image)), name)
        }
    }
}

/// The No Git marker of a folder project's row (WS-02): a warning badge with its word, found by its selector.
fn no_git_marker(project_id: &str) -> impl IntoElement {
    let selector = format!("sidebar-no-git-{project_id}");
    div()
        .debug_selector(move || selector)
        .child(Badge::text("No Git").tone(BadgeTone::Warning))
}

/// One row of the project tree, identified by its position in the list.
pub(super) fn tree_row(
    row: &TreeRow,
    index: usize,
    focus: &FocusHandle,
    shell: &WeakEntity<Shell>,
) -> SidebarRow {
    let id = SharedString::from(format!("tree-row-{index}"));
    let item = match row {
        TreeRow::Project {
            id: project_id,
            name,
            icon,
            owner,
            plain_folder,
            expanded,
        } => {
            let (click_shell, click_id) = (shell.clone(), project_id.clone());
            let (key_shell, key_id) = (shell.clone(), project_id.clone());
            // A click on the row, or Left and Right on the keyboard, folds the project's rows (A10).
            let row = project_row(id, name, icon)
                .expanded(*expanded)
                .on_click(move |_, _, cx| {
                    let id = click_id.clone();
                    change(&click_shell, cx, move |shell| shell.toggle_project(&id));
                })
                .on_toggle(move |_, _, cx| {
                    let id = key_id.clone();
                    change(&key_shell, cx, move |shell| shell.toggle_project(&id));
                });
            let row = if *plain_folder {
                row.trailing(no_git_marker(project_id))
            } else {
                row
            };
            // The owner or org reads after the name, in the secondary text (A10).
            match owner {
                Some(owner) => row.secondary(owner.clone()),
                None => row,
            }
        }
        TreeRow::Host { name, reach } => host_row(id, name, *reach),
        TreeRow::RemoteHost { name, state } => remote_host_row(id, name, state),
        TreeRow::Session {
            id: session_id,
            title,
            agent,
            state,
            depth,
            selected,
            unread,
            can_bring_back,
            tooltip,
            workspace_id,
            dimmed,
            local,
            host,
        } => {
            let mut row = session_row(id, title, *agent, *state, *depth, *selected);
            if let Some(tooltip) = tooltip {
                row = row.tooltip(tooltip.clone());
            }
            if *dimmed {
                row = row.dimmed(true);
            }
            // An unread session shows an accent dot, and its title in body-strong (WS-33).
            let row = if *unread {
                row.title_strong(true)
                    .trailing(Badge::dot().tone(BadgeTone::Accent))
            } else {
                row
            };
            // A click opens the session, and tells the service it was seen when it was unread (WS-33).
            let (open_shell, open_id) = (shell.clone(), session_id.clone());
            let row = row.on_click(move |_, _, cx| {
                let id = open_id.clone();
                let _ = open_shell.update(cx, |shell, cx| {
                    shell.open_session(&id, cx);
                    cx.notify();
                });
            });
            // Right click or Shift+F10 opens the session's menu (A97).
            let (menu_shell, menu_session, menu_workspace) =
                (shell.clone(), session_id.clone(), workspace_id.clone());
            let local = *local;
            let remote_host = host.clone();
            let row = row.on_context_menu(move |window, cx| {
                let (session, workspace) = (menu_session.clone(), menu_workspace.clone());
                let _ = menu_shell.update(cx, |shell, cx| match &remote_host {
                    // A session a paired host runs has its menu over that host's connection (STAT-16).
                    Some(host) => shell.open_remote_session_menu(host.clone(), session, window, cx),
                    None => shell.open_session_menu(Some(session), workspace, local, window, cx),
                });
            });
            if *can_bring_back {
                row.hover_action(bring_back_action(session_id, shell))
            } else {
                row
            }
        }
        TreeRow::Placeholder {
            workspace_id,
            title,
            agent,
            tooltip,
            dimmed,
            local,
        } => {
            let mut row =
                SidebarRow::session(id, title.clone(), GlyphState::Idle, agent_kind(*agent))
                    .level(SESSION_LEVEL);
            if let Some(tooltip) = tooltip {
                row = row.tooltip(tooltip.clone());
            }
            if *dimmed {
                row = row.dimmed(true);
            }
            // Right click opens the workspace's menu while it has no session, as a session row does (A97).
            let (menu_shell, menu_workspace) = (shell.clone(), workspace_id.clone());
            let local = *local;
            row.on_context_menu(move |window, cx| {
                let workspace = menu_workspace.clone();
                let _ = menu_shell.update(cx, |shell, cx| {
                    shell.open_session_menu(None, workspace, local, window, cx);
                });
            })
        }
    };
    item.focus_handle(focus.clone()).tab_stop(true)
}

/// One tree row as an element. Each row is padded to its indent (A97), and a session carries a debug
/// selector, so tests can click it.
pub(super) fn tree_item(
    row: &TreeRow,
    index: usize,
    focus: &FocusHandle,
    shell: &WeakEntity<Shell>,
) -> AnyElement {
    let item = tree_row(row, index, focus, shell);
    let wrapper = div().pl(px(indent_padding(row)));
    // A project with an owner draws its owner line inside a selector of its own, so a rig can see the line.
    let item = match owner_selector(row) {
        Some(selector) => div()
            .debug_selector(move || selector)
            .child(item)
            .into_any_element(),
        None => item.into_any_element(),
    };
    match row_selector(row) {
        Some(selector) => wrapper
            .debug_selector(move || selector)
            .child(item)
            .into_any_element(),
        None => wrapper.child(item).into_any_element(),
    }
}

/// The padding that brings a row to its indent. The row's own level indent is part of its indent, so
/// this is the rest: the indent less the list padding and the level's step.
fn indent_padding(row: &TreeRow) -> f32 {
    let (indent, level) = match row {
        TreeRow::Project { .. } => return 0.0,
        TreeRow::Host { .. } | TreeRow::RemoteHost { .. } => (HOST_INDENT, HOST_LEVEL),
        TreeRow::Session { depth, .. } => (session_indent(*depth), SESSION_LEVEL + depth),
        TreeRow::Placeholder { .. } => (session_indent(0), SESSION_LEVEL),
    };
    indent - SPACE_2 - level as f32 * TREE_INDENT
}

/// The debug selector of a project's owner line, which only a project with an owner has.
fn owner_selector(row: &TreeRow) -> Option<String> {
    match row {
        TreeRow::Project {
            id, owner: Some(_), ..
        } => Some(format!("sidebar-project-owner-{id}")),
        _ => None,
    }
}

/// The debug selector of a row a test clicks: a session by its id.
fn row_selector(row: &TreeRow) -> Option<String> {
    match row {
        TreeRow::Project { id, .. } => Some(format!("sidebar-project-{id}")),
        TreeRow::Session { id, .. } => Some(format!("sidebar-session-{id}")),
        TreeRow::Placeholder { workspace_id, .. } => {
            Some(format!("sidebar-placeholder-{workspace_id}"))
        }
        _ => None,
    }
}

/// The row action that opens the bring-back confirm for a finished child. It shows on hover or focus.
fn bring_back_action(session: &str, shell: &WeakEntity<Shell>) -> IconButton {
    let (shell, session) = (shell.clone(), session.to_owned());
    IconButton::new(
        SharedString::from(format!("bring-back-{session}")),
        IconName::GitBranch,
        "Bring back",
    )
    .on_click(move |_, cx| {
        let _ = shell.update(cx, |shell, cx| shell.ask_bring_back(&session, cx));
    })
}

/// A host row. An unreachable host dims only itself and shows how long ago it was reached (A10).
fn host_row(id: SharedString, name: &str, reach: HostReach) -> SidebarRow {
    let row = match reach {
        HostReach::Connected => SidebarRow::host(id, name.to_owned(), HostConnection::Connected),
        HostReach::Unreachable { minutes } => {
            SidebarRow::host(id, name.to_owned(), HostConnection::Unreachable)
                .secondary(format!("Last known state, {minutes} min ago"))
        }
    };
    row.level(HOST_LEVEL)
}

/// A paired remote host below the projects (D-HO-01). Only a connected host gets the connected glyph; the
/// line under it says the state, or how long ago the host was last reached (X-CN-01).
fn remote_host_row(
    id: SharedString,
    name: &str,
    state: &crate::model::hosts::HostState,
) -> SidebarRow {
    use crate::model::hosts::HostTone;
    let connection = match state.tone {
        HostTone::Connected => HostConnection::Connected,
        _ => HostConnection::Unreachable,
    };
    let line = match (state.tone, &state.detail) {
        (HostTone::Unreachable, Some(detail)) => detail.clone(),
        _ => state.word.to_owned(),
    };
    SidebarRow::host(id, name.to_owned(), connection)
        .secondary(line)
        .level(HOST_LEVEL)
}

/// A session row: its status glyph, its agent's mark, then its title (A84, A97). Nothing else shows on
/// the row; the worktree and branch live in its tooltip.
fn session_row(
    id: SharedString,
    title: &str,
    agent: Agent,
    state: AgentState,
    depth: usize,
    selected: bool,
) -> SidebarRow {
    SidebarRow::session(id, title.to_owned(), glyph_state(state), agent_kind(agent))
        .level(SESSION_LEVEL + depth)
        .selected(selected)
}

/// The icon of each navigation row. Two Lucide names are not bundled yet and use the nearest
/// bundled icon: Prototypes (`layers`) and Automations (`calendar-clock`).
fn nav_icon(kind: NavKind) -> IconName {
    match kind {
        NavKind::Prototypes => IconName::Layers,
        NavKind::Tasks => IconName::ListChecks,
        NavKind::Automations => IconName::CalendarClock,
        NavKind::Mobile => IconName::Smartphone,
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
