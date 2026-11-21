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
    if let Some(page) = row.page {
        let shell = shell.clone();
        item = item.on_click(move |_, _, cx| {
            change(&shell, cx, move |shell| shell.model.open_page(page));
        });
    }
    item
}

/// One row of the project tree, identified by its position in the list. `workspaces` counts the
/// workspace rows above it, which picks the usage colour of a workspace label (P29).
pub(super) fn tree_row(
    row: &TreeRow,
    index: usize,
    workspaces: usize,
    focus: &FocusHandle,
) -> SidebarRow {
    let id = SharedString::from(format!("tree-row-{index}"));
    let item = match row {
        TreeRow::Project { name } => SidebarRow::project(id, IconName::Folder, name.clone()),
        TreeRow::Host { name, reach } => host_row(id, name, *reach),
        TreeRow::Workspace { name, branch } => {
            let mut item = SidebarRow::workspace(
                id,
                name.clone(),
                WorkspaceKind::Worktree,
                usage_slot(workspaces),
            )
            .level(WORKSPACE_LEVEL);
            if let Some(branch) = branch {
                item = item.secondary(branch.clone());
            }
            item
        }
        TreeRow::Session {
            title,
            agent,
            state,
            depth,
            selected,
        } => session_row(id, title, *agent, *state, *depth, *selected),
    };
    item.focus_handle(focus.clone()).tab_stop(true)
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

/// A session: its status glyph, its agent's mark, then its title (A84). A child session sits one
/// step deeper than its parent, so its agent mark lines up with the parent's title (A83, A85).
fn session_row(
    id: SharedString,
    title: &str,
    agent: Agent,
    state: AgentState,
    depth: usize,
    selected: bool,
) -> SidebarRow {
    let row = SidebarRow::session(id, title.to_owned(), glyph_state(state), agent_kind(agent))
        .level(SESSION_LEVEL + depth)
        .selected(selected);
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

/// The usage colour of the `ordinal`th workspace, the six slots in turn (P29). The view model
/// does not carry a slot per workspace yet.
fn usage_slot(ordinal: usize) -> UsageSlot {
    UsageSlot::new((ordinal % 6) as u8 + 1).unwrap_or_default()
}

/// The icon of each navigation row. Two Lucide names are not bundled yet and use the nearest
/// bundled icon: Prototypes (`layers`) and Automations (`calendar-clock`).
fn nav_icon(kind: NavKind) -> IconName {
    match kind {
        NavKind::Prototypes => IconName::Layers,
        NavKind::Tasks => IconName::ListChecks,
        NavKind::Automations => IconName::CalendarClock,
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
