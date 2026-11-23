//! Human-readable lines for projects and workspaces: key-value blocks and aligned tables.

use massorca_protocol::methods::projects::{Project, ProjectKind};
use massorca_protocol::methods::workspaces::{
    UnregisteredWorktree, Workspace, WorkspaceKind, WorkspaceRemoved, WorkspaceState,
};

pub fn project_kind(kind: ProjectKind) -> &'static str {
    match kind {
        ProjectKind::Git => "git",
        ProjectKind::Folder => "folder",
        ProjectKind::Unknown => "unknown",
    }
}

pub fn workspace_kind(kind: WorkspaceKind) -> &'static str {
    match kind {
        WorkspaceKind::Worktree => "worktree",
        WorkspaceKind::Folder => "folder",
        WorkspaceKind::Unknown => "unknown",
    }
}

/// The state, with `missing` added when the directory is gone.
pub fn workspace_state(workspace: &Workspace) -> String {
    let state = match workspace.state {
        WorkspaceState::Ready => "ready",
        WorkspaceState::Removing => "removing",
        WorkspaceState::Unknown => "unknown",
    };
    if workspace.missing {
        format!("{state}, missing")
    } else {
        state.to_owned()
    }
}

/// One line for a project that was just added or removed.
pub fn project_line(project: &Project, verb: &str) -> String {
    format!(
        "{verb} project {} ({}) at {}",
        project.name,
        project_kind(project.kind),
        project.path
    )
}

/// One line for a project whose base was set or cleared.
pub fn project_base_line(project: &Project) -> String {
    match &project.base_ref {
        Some(base) => format!("project {} base is {base}", project.name),
        None => format!("project {} has no base", project.name),
    }
}

/// The fields of one project, one per line; the base only when one is set.
pub fn project_details(project: &Project) -> Vec<String> {
    let mut lines = vec![
        format!("id: {}", project.id),
        format!("name: {}", project.name),
        format!("kind: {}", project_kind(project.kind)),
        format!("path: {}", project.path),
        format!("workspaces root: {}", project.workspaces_root),
    ];
    if let Some(base) = &project.base_ref {
        lines.push(format!("base: {base}"));
    }
    lines
}

pub fn project_table(projects: &[Project]) -> Vec<String> {
    let rows = projects
        .iter()
        .map(|project| {
            vec![
                project.id.clone(),
                project.name.clone(),
                project_kind(project.kind).to_owned(),
                project.base_ref.clone().unwrap_or_else(|| "-".to_owned()),
                project.path.clone(),
            ]
        })
        .collect();
    table(&["ID", "NAME", "KIND", "BASE", "PATH"], rows)
}

/// One line for a workspace that was just created.
pub fn workspace_line(workspace: &Workspace, verb: &str) -> String {
    format!(
        "{verb} workspace {} ({}) at {}",
        workspace.name,
        workspace_kind(workspace.kind),
        workspace.path
    )
}

/// The fields of one workspace, one per line; branch and base appear only when set.
pub fn workspace_details(workspace: &Workspace) -> Vec<String> {
    let mut lines = vec![
        format!("id: {}", workspace.id),
        format!("name: {}", workspace.name),
        format!("project: {}", workspace.project_id),
        format!("kind: {}", workspace_kind(workspace.kind)),
    ];
    if let Some(branch) = &workspace.branch {
        lines.push(format!("branch: {branch}"));
    }
    if let Some(base) = &workspace.base_ref {
        lines.push(format!("base: {base}"));
    }
    lines.push(format!("path: {}", workspace.path));
    lines.push(format!("state: {}", workspace_state(workspace)));
    lines
}

pub fn workspace_table(workspaces: &[Workspace]) -> Vec<String> {
    let rows = workspaces
        .iter()
        .map(|workspace| {
            vec![
                workspace.id.clone(),
                workspace.name.clone(),
                workspace_kind(workspace.kind).to_owned(),
                workspace.branch.clone().unwrap_or_else(|| "-".to_owned()),
                workspace.path.clone(),
                workspace_state(workspace),
            ]
        })
        .collect();
    table(&["ID", "NAME", "KIND", "BRANCH", "PATH", "STATE"], rows)
}

/// What a removal did: the workspace, the branch when it was deleted or kept, and a directory left in place.
pub fn removal_lines(removed: &WorkspaceRemoved) -> Vec<String> {
    let workspace = &removed.workspace;
    let mut lines = vec![format!("removed workspace {}", workspace.name)];
    if removed.branch_deleted {
        if let Some(branch) = &workspace.branch {
            lines.push(format!("deleted branch {branch}"));
        }
    } else if let (Some(branch), Some(reason)) = (&workspace.branch, &removed.branch_kept_reason) {
        lines.push(format!("kept branch {branch}: {reason}"));
    }
    if removed.directory_kept {
        lines.push(format!("kept directory {}", workspace.path));
    }
    lines
}

/// A header and rows, each column padded to its widest cell and separated by two spaces.
pub fn table(headers: &[&str], rows: Vec<Vec<String>>) -> Vec<String> {
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.len());
        }
    }
    let format_row = |cells: Vec<&str>| -> String {
        let line = cells
            .iter()
            .zip(&widths)
            .map(|(cell, width)| format!("{cell:<width$}"))
            .collect::<Vec<_>>()
            .join("  ");
        line.trim_end().to_owned()
    };
    let mut lines = vec![format_row(headers.to_vec())];
    lines.extend(
        rows.iter()
            .map(|row| format_row(row.iter().map(String::as_str).collect())),
    );
    lines
}

/// One line for a worktree under a workspaces root that no workspace records. It is reported
/// only: a create with the same name and branch adopts it.
pub fn unregistered_line(worktree: &UnregisteredWorktree) -> String {
    format!(
        "unregistered worktree {} (branch {}): not a workspace; a create with this name and branch adopts it",
        worktree.path,
        worktree.branch.as_deref().unwrap_or("-")
    )
}
