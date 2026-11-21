use massorca_core_store::{
    ProjectKind, ProjectRow, StoreError, WorkspaceKind, WorkspaceRow, WorkspaceState, new_id,
    now_ms,
};

/// A git project row with a fresh id.
pub fn project_row(name: &str, path_key: &str) -> ProjectRow {
    ProjectRow {
        id: new_id(),
        kind: ProjectKind::Git,
        path: format!("/repos/{name}"),
        path_key: path_key.to_owned(),
        name: name.to_owned(),
        workspaces_root: format!("/repos/{name}-workspaces"),
        added_at: now_ms(),
    }
}

/// A ready worktree row with a fresh id, created now.
pub fn workspace_row(project_id: &str, name: &str, path_key: &str) -> WorkspaceRow {
    WorkspaceRow {
        id: new_id(),
        project_id: project_id.to_owned(),
        kind: WorkspaceKind::Worktree,
        name: name.to_owned(),
        path: format!("/repos/{name}-workspaces/{name}"),
        path_key: path_key.to_owned(),
        branch: Some(name.to_owned()),
        base_ref: Some("main".to_owned()),
        created_branch: true,
        state: WorkspaceState::Ready,
        request_id: None,
        created_at: now_ms(),
    }
}

/// Asserts a unique-constraint conflict whose message names `expected` (for example `projects.name`).
pub fn assert_conflict(result: Result<(), StoreError>, expected: &str) {
    match result {
        Err(StoreError::Conflict { constraint }) => {
            assert!(
                constraint.contains(expected),
                "expected {expected}, got {constraint}"
            );
        }
        other => panic!("expected a conflict on {expected}, got {other:?}"),
    }
}
