//! Human-readable lines for projects and workspaces: key-value blocks and aligned tables.

use massorca_protocol::methods::projects::{
    Project, ProjectIcon, ProjectIconSource, ProjectInspection, ProjectKind,
};
use massorca_protocol::methods::workspaces::{
    UnregisteredWorktree, Workspace, WorkspaceCheck, WorkspaceKind, WorkspaceRemoved,
    WorkspaceState,
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

/// The warning `project add` prints when a directory without a kind became a plain folder.
pub fn plain_folder_warning(path: &str) -> String {
    format!("warning: {path} is not a git repository; it was added as a plain folder")
}

/// What `project add` without a kind would register, and the project already registered there.
pub fn inspection_lines(inspection: &ProjectInspection) -> Vec<String> {
    let kind = match inspection.kind {
        ProjectKind::Git => "git repository".to_owned(),
        ProjectKind::Folder => {
            "not a git repository; it would be added as a plain folder".to_owned()
        }
        ProjectKind::Unknown => "unknown kind".to_owned(),
    };
    let mut lines = vec![format!("{}: {kind}", inspection.path)];
    if let Some(id) = &inspection.registered {
        lines.push(format!("already registered as {id}"));
    }
    lines
}

/// One line for a project whose base was set or cleared.
pub fn project_base_line(project: &Project) -> String {
    match &project.base_ref {
        Some(base) => format!("project {} base is {base}", project.name),
        None => format!("project {} has no base", project.name),
    }
}

/// What `project rm --forget-workspaces` dropped. No file, folder, worktree or branch is touched.
pub fn forgotten_line(count: usize) -> String {
    let noun = if count == 1 { "record" } else { "records" };
    format!(
        "forgot {count} workspace {noun}; no files, folders, worktrees or branches were touched"
    )
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
    if let Some(icon) = &project.icon {
        lines.push(format!("icon: {}", icon_text(icon)));
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
                project
                    .icon
                    .as_ref()
                    .map_or_else(|| "-".to_owned(), icon_text),
                project_kind(project.kind).to_owned(),
                project.base_ref.clone().unwrap_or_else(|| "-".to_owned()),
                project.path.clone(),
            ]
        })
        .collect();
    table(&["ID", "NAME", "ICON", "KIND", "BASE", "PATH"], rows)
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

/// One line for a check: `name: ok` or `name: the first failure`, then the path.
pub fn check_line(check: &WorkspaceCheck) -> String {
    let status = check.reason.as_deref().unwrap_or("ok");
    format!("{}: {status} ({})", check.name, check.path)
}

/// The fields of one workspace, one per line; branch, base, comment, lock and prune appear only when set.
pub fn workspace_details(workspace: &Workspace) -> Vec<String> {
    let mut lines = vec![
        format!("id: {}", workspace.id),
        format!("name: {}", workspace.name),
        format!(
            "name generated: {}",
            if workspace.name_generated {
                "yes"
            } else {
                "no"
            }
        ),
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
    if let Some(comment) = &workspace.comment {
        lines.push(format!("comment: {comment}"));
    }
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

/// A project's icon as `emoji:<emoji>`, `lucide:<name>` or `image:<label> (upload|file)`, read as the forms `--icon` takes.
pub fn icon_text(icon: &ProjectIcon) -> String {
    match icon {
        ProjectIcon::Emoji { emoji } => format!("emoji:{emoji}"),
        ProjectIcon::Lucide { name } => format!("lucide:{name}"),
        ProjectIcon::Image { source, label, .. } => {
            let origin = match source {
                ProjectIconSource::Upload => "upload",
                ProjectIconSource::File => "file",
            };
            format!("image:{} ({origin})", label.as_deref().unwrap_or("image"))
        }
    }
}

/// One line for a project's icon, or for its lack of one.
pub fn project_icon_line(project: &Project) -> String {
    match &project.icon {
        Some(icon) => format!("project {} icon is {}", project.name, icon_text(icon)),
        None => format!("project {} has no icon", project.name),
    }
}
