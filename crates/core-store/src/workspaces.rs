use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// Whether a workspace is a git worktree or a plain folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceKind {
    Worktree,
    Folder,
}

impl WorkspaceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Worktree => "worktree",
            Self::Folder => "folder",
        }
    }
}

impl ToSql for WorkspaceKind {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

impl FromSql for WorkspaceKind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "worktree" => Ok(Self::Worktree),
            "folder" => Ok(Self::Folder),
            other => Err(FromSqlError::Other(
                format!("unknown workspace kind {other:?}").into(),
            )),
        }
    }
}

/// Whether a workspace is usable or being removed. `Removing` is written before any file is deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceState {
    Ready,
    Removing,
}

impl WorkspaceState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Removing => "removing",
        }
    }
}

impl ToSql for WorkspaceState {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

impl FromSql for WorkspaceState {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "ready" => Ok(Self::Ready),
            "removing" => Ok(Self::Removing),
            other => Err(FromSqlError::Other(
                format!("unknown workspace state {other:?}").into(),
            )),
        }
    }
}

/// One row of `workspaces`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRow {
    pub id: String,
    pub project_id: String,
    pub kind: WorkspaceKind,
    pub name: String,
    pub path: String,
    pub path_key: String,
    pub branch: Option<String>,
    pub base_ref: Option<String>,
    pub created_branch: bool,
    pub state: WorkspaceState,
    pub request_id: Option<String>,
    pub created_at: i64,
    /// The owner's short comment, set by `workspace.update`; `None` until one is set.
    pub comment: Option<String>,
}

const SELECT_WORKSPACES: &str = "SELECT id, project_id, kind, name, path, path_key, branch, base_ref, created_branch, state, request_id, created_at, comment FROM workspaces";

fn workspace_from_row(row: &Row<'_>) -> rusqlite::Result<WorkspaceRow> {
    Ok(WorkspaceRow {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        kind: row.get("kind")?,
        name: row.get("name")?,
        path: row.get("path")?,
        path_key: row.get("path_key")?,
        branch: row.get("branch")?,
        base_ref: row.get("base_ref")?,
        created_branch: row.get("created_branch")?,
        state: row.get("state")?,
        request_id: row.get("request_id")?,
        created_at: row.get("created_at")?,
        comment: row.get("comment")?,
    })
}

impl Store {
    /// Inserts a workspace. Conflicts on id, path key, request id or (project, name) are `StoreError::Conflict`.
    pub fn insert_workspace(&self, row: &WorkspaceRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO workspaces (id, project_id, kind, name, path, path_key, branch, base_ref, created_branch, state, request_id, created_at, comment) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                row.id,
                row.project_id,
                row.kind,
                row.name,
                row.path,
                row.path_key,
                row.branch,
                row.base_ref,
                row.created_branch,
                row.state,
                row.request_id,
                row.created_at,
                row.comment
            ],
        )?;
        Ok(())
    }

    pub fn get_workspace(&self, id: &str) -> Result<Option<WorkspaceRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_WORKSPACES} WHERE id = ?1"),
            [id],
            workspace_from_row,
        )
    }

    pub fn find_workspace_by_name(
        &self,
        project_id: &str,
        name: &str,
    ) -> Result<Option<WorkspaceRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_WORKSPACES} WHERE project_id = ?1 AND name = ?2"),
            [project_id, name],
            workspace_from_row,
        )
    }

    pub fn find_workspace_by_path_key(
        &self,
        path_key: &str,
    ) -> Result<Option<WorkspaceRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_WORKSPACES} WHERE path_key = ?1"),
            [path_key],
            workspace_from_row,
        )
    }

    pub fn find_workspace_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Option<WorkspaceRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_WORKSPACES} WHERE request_id = ?1"),
            [request_id],
            workspace_from_row,
        )
    }

    /// Workspaces ordered by creation time, optionally limited to one project.
    pub fn list_workspaces(
        &self,
        project_id: Option<&str>,
    ) -> Result<Vec<WorkspaceRow>, StoreError> {
        self.query_all(
            &format!(
                "{SELECT_WORKSPACES} WHERE (?1 IS NULL OR project_id = ?1) ORDER BY created_at, id"
            ),
            [project_id],
            workspace_from_row,
        )
    }

    /// Returns whether a row changed.
    pub fn set_workspace_state(&self, id: &str, state: WorkspaceState) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE workspaces SET state = ?2 WHERE id = ?1",
            params![id, state],
        )?;
        Ok(changed > 0)
    }

    /// Sets a workspace's name and comment in one write; a `None` comment clears it. Returns whether a row changed.
    /// A name another workspace of the project holds is `StoreError::Conflict`.
    pub fn update_workspace(
        &self,
        id: &str,
        name: &str,
        comment: Option<&str>,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE workspaces SET name = ?2, comment = ?3 WHERE id = ?1",
            params![id, name, comment],
        )?;
        Ok(changed > 0)
    }

    /// Lets several agents write in a folder workspace at once. Returns false for a worktree or an unknown id: a git
    /// index is never shared.
    pub fn share_workspace_writers(&self, id: &str) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE workspaces SET shared_writers = 1 WHERE id = ?1 AND kind = 'folder'",
            [id],
        )?;
        Ok(changed > 0)
    }

    /// Whether several agents may write in the workspace at once.
    pub fn workspace_shares_writers(&self, id: &str) -> Result<bool, StoreError> {
        Ok(self
            .query_optional(
                "SELECT shared_writers FROM workspaces WHERE id = ?1",
                [id],
                |row| row.get::<_, bool>("shared_writers"),
            )?
            .unwrap_or(false))
    }

    /// Returns whether a row was removed.
    pub fn delete_workspace(&self, id: &str) -> Result<bool, StoreError> {
        let removed = self
            .lock()
            .execute("DELETE FROM workspaces WHERE id = ?1", [id])?;
        Ok(removed > 0)
    }

    pub fn count_workspaces(&self, project_id: &str) -> Result<u64, StoreError> {
        let count: i64 = self.lock().query_row(
            "SELECT COUNT(*) FROM workspaces WHERE project_id = ?1",
            [project_id],
            |row| row.get(0),
        )?;
        // COUNT(*) is never negative, so the fallback is unreachable.
        Ok(u64::try_from(count).unwrap_or(0))
    }
}
