use massorca_core_store::{ProjectKind, ProjectRow, StoreError, new_id, now_ms};

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
