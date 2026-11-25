//! Rows of the Sidebar: maps the navigation rows and the project tree of the view model onto the
//! shared SidebarRow (A10, A83 to A85, A84, A97, P29).

use super::Shell;
use super::change;
use super::rail::agent_kind;
use crate::model::shell::{Agent, AgentState, HostReach};
use crate::model::sidebar::{HOST_INDENT, NavKind, NavRow, TreeRow, project_label, session_indent};
use gpui::{AnyElement, FocusHandle, SharedString, WeakEntity, div, prelude::*, px};
use massorca_ui::IconName;
use massorca_ui::components::agent_status::AgentState as GlyphState;
use massorca_ui::components::badge::{Badge, BadgeTone};
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::components::list_row::TREE_INDENT;
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

/// One row of the project tree, identified by its position in the list.
pub(super) fn tree_row(
    row: &TreeRow,
    index: usize,
    focus: &FocusHandle,
    shell: &WeakEntity<Shell>,
) -> SidebarRow {
    let id = SharedString::from(format!("tree-row-{index}"));
    let item = match row {
        TreeRow::Project { name, emoji } => {
            SidebarRow::project(id, IconName::Folder, project_label(name, emoji.as_deref()))
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
            dimmed,
            ..
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
            if *can_bring_back {
                row.hover_action(bring_back_action(session_id, shell))
            } else {
                row
            }
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
    };
    indent - SPACE_2 - level as f32 * TREE_INDENT
}

/// The debug selector of a row a test clicks: a session by its id.
fn row_selector(row: &TreeRow) -> Option<String> {
    match row {
        TreeRow::Session { id, .. } => Some(format!("sidebar-session-{id}")),
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
