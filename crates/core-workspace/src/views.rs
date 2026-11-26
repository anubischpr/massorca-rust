//! Store rows as protocol types. `missing` is answered by the directory itself (ADR 0008),
//! never by git's prunable line.

use std::path::Path;

use massorca_core_git::WorktreeEntry;
use massorca_core_store::{ProjectKind, ProjectRow, WorkspaceKind, WorkspaceRow, WorkspaceState};
use massorca_protocol::methods::projects::{self, Project, ProjectIcon};
use massorca_protocol::methods::workspaces::{self, Workspace};

pub(crate) fn project(row: &ProjectRow) -> Project {
    Project {
        id: row.id.clone(),
        kind: project_kind(row.kind),
        path: row.path.clone(),
        name: row.name.clone(),
        workspaces_root: row.workspaces_root.clone(),
        added_at_ms: row.added_at,
        base_ref: row.base_ref.clone(),
        icon: row.icon.as_deref().and_then(ProjectIcon::from_stored),
    }
}

pub(crate) fn project_kind(kind: ProjectKind) -> projects::ProjectKind {
    match kind {
        ProjectKind::Git => projects::ProjectKind::Git,
        ProjectKind::Folder => projects::ProjectKind::Folder,
    }
}

pub(crate) fn workspace(row: &WorkspaceRow) -> Workspace {
    Workspace {
        id: row.id.clone(),
        project_id: row.project_id.clone(),
        kind: workspace_kind(row.kind),
        name: row.name.clone(),
        path: row.path.clone(),
        branch: row.branch.clone(),
        base_ref: row.base_ref.clone(),
        created_branch: row.created_branch,
        state: match row.state {
            WorkspaceState::Ready => workspaces::WorkspaceState::Ready,
            WorkspaceState::Removing => workspaces::WorkspaceState::Removing,
        },
        missing: !Path::new(&row.path).is_dir(),
        created_at_ms: row.created_at,
        locked: None,
        prunable: None,
        base_refresh: None,
        comment: row.comment.clone(),
        setup: None,
        name_generated: row.name_generated,
    }
}

/// The protocol kind of a store kind.
pub(crate) fn workspace_kind(kind: WorkspaceKind) -> workspaces::WorkspaceKind {
    match kind {
        WorkspaceKind::Worktree => workspaces::WorkspaceKind::Worktree,
        WorkspaceKind::Folder => workspaces::WorkspaceKind::Folder,
    }
}

/// The workspace with git's lock and prune state of its worktree.
pub(crate) fn with_worktree_state(mut workspace: Workspace, entry: &WorktreeEntry) -> Workspace {
    workspace.locked = entry.locked.clone();
    workspace.prunable = entry.prunable.clone();
    workspace
}
