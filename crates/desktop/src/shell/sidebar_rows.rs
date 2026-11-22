//! Rows of the Sidebar: maps the navigation rows and the project tree of the view model onto the
//! shared SidebarRow (A10, A83 to A85, A84, P29).

use super::Shell;
use super::change;
use super::rail::agent_kind;
use crate::model::shell::{Agent, AgentState, HostReach};
use crate::model::sidebar::{NavKind, NavRow, TreeRow};
use gpui::{FocusHandle, SharedString, WeakEntity};
use massorca_ui::IconName;
use massorca_ui::components::agent_status::AgentState as GlyphState;
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::components::list_row::TREE_INDENT;
use massorca_ui::components::sidebar_row::{
    HostConnection, SidebarRow, WorkspaceKind, child_session_indent,
};
use massorca_ui::components::usage_ring::UsageSlot;

/// The tree level of each kind below the project: host 1, workspace 2, session 3 (A10, A83).
const HOST_LEVEL: usize = 1;
const WORKSPACE_LEVEL: usize = 2;
const SESSION_LEVEL: usize = 3;

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
        TreeRow::Project { name } => SidebarRow::project(id, IconName::Folder, name.clone()),
        TreeRow::Host { name, reach } => host_row(id, name, *reach),
        TreeRow::RemoteHost { name, state } => remote_host_row(id, name, state),
        TreeRow::Workspace {
            name,
            branch,
            id: workspace_id,
            usage_slot,
        } => {
            let mut item = SidebarRow::workspace(
                id,
                name.clone(),
                WorkspaceKind::Worktree,
                UsageSlot::new(*usage_slot).unwrap_or_default(),
            )
            .level(WORKSPACE_LEVEL);
            if let Some(branch) = branch {
                item = item.secondary(branch.clone());
            }
            // A click opens the start page; right click or Shift+F10 offers to remove it.
            let (open_shell, open_id) = (shell.clone(), workspace_id.clone());
            let item = item.on_click(move |_, _, cx| {
                let id = open_id.clone();
                change(&open_shell, cx, move |shell| shell.model.open_workspace(id));
            });
            let (shell, workspace_id) = (shell.clone(), workspace_id.clone());
            item.on_context_menu(move |window, cx| {
                let id = workspace_id.clone();
                let _ = shell.update(cx, |shell, cx| shell.open_workspace_menu(id, window, cx));
            })
        }
        TreeRow::Session {
            id: session_id,
            title,
            agent,
            state,
            depth,
            selected,
            place,
            can_bring_back,
        } => {
            let row = session_row(
                id,
                title,
                *agent,
                *state,
                *depth,
                *selected,
                place.as_deref(),
            );
            if *can_bring_back {
                row.hover_action(bring_back_action(session_id, shell))
            } else {
                row
            }
        }
    };
    item.focus_handle(focus.clone()).tab_stop(true)
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

/// A session: its status glyph, its agent's mark, then its title (A84). A child session sits one
/// step deeper than its parent, so its agent mark lines up with the parent's title (A83, A85).
fn session_row(
    id: SharedString,
    title: &str,
    agent: Agent,
    state: AgentState,
    depth: usize,
    selected: bool,
    place: Option<&str>,
) -> SidebarRow {
    let mut row = SidebarRow::session(id, title.to_owned(), glyph_state(state), agent_kind(agent))
        .level(SESSION_LEVEL + depth)
        .selected(selected);
    if let Some(place) = place {
        row = row.secondary(place.to_owned());
    }
    if depth == 0 {
        row
    } else {
        row.child_of(session_indent(depth - 1))
    }
}

/// The indent of a session `depth` child steps below a top-level session.
fn session_indent(depth: usize) -> f32 {
    (0..depth).fold(SESSION_LEVEL as f32 * TREE_INDENT, |parent, _| {
        child_session_indent(parent)
    })
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
