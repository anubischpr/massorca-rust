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

/// The state, with `missing`, and git's `locked` and `prunable` with their reasons.
pub fn workspace_state(workspace: &Workspace) -> String {
    let mut parts = vec![state_word(workspace)];
    if let Some(reason) = &workspace.locked {
        parts.push(reason_text("locked", reason));
    }
    if let Some(reason) = &workspace.prunable {
        parts.push(reason_text("prunable", reason));
    }
    parts.join(", ")
}

/// The state word, with `missing` added when the directory is gone.
fn state_word(workspace: &Workspace) -> String {
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

/// `word: reason`, or `word` alone when git gives no reason.
fn reason_text(word: &str, reason: &str) -> String {
    if reason.is_empty() {
        word.to_owned()
    } else {
        format!("{word}: {reason}")
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

/// What `project rm --forget-workspaces` dropped. No file, folder, worktree or branch is touched.
pub fn forgotten_line(count: usize) -> String {
    let noun = if count == 1 { "record" } else { "records" };
    format!(
        "forgot {count} workspace {noun}; no files, folders, worktrees or branches were touched"
    )
}

/// The fields of one project, one per line.
pub fn project_details(project: &Project) -> Vec<String> {
    vec![
        format!("id: {}", project.id),
        format!("name: {}", project.name),
        format!("kind: {}", project_kind(project.kind)),
        format!("path: {}", project.path),
        format!("workspaces root: {}", project.workspaces_root),
    ]
}

pub fn project_table(projects: &[Project]) -> Vec<String> {
    let rows = projects
        .iter()
        .map(|project| {
            vec![
                project.id.clone(),
                project.name.clone(),
                project_kind(project.kind).to_owned(),
                project.path.clone(),
            ]
        })
        .collect();
    table(&["ID", "NAME", "KIND", "PATH"], rows)
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

/// The fields of one workspace, one per line; branch, base, lock and prune appear only when set.
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
    lines.push(format!("state: {}", state_word(workspace)));
    if let Some(reason) = &workspace.locked {
        lines.push(reason_text("locked", reason));
    }
    if let Some(reason) = &workspace.prunable {
        lines.push(reason_text("prunable", reason));
    }
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
