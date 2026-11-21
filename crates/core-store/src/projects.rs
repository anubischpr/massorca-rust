use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// Whether a project is a git repository or a plain folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    Git,
    Folder,
}

impl ProjectKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Folder => "folder",
        }
    }
}

impl ToSql for ProjectKind {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

impl FromSql for ProjectKind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "git" => Ok(Self::Git),
            "folder" => Ok(Self::Folder),
            other => Err(FromSqlError::Other(
                format!("unknown project kind {other:?}").into(),
            )),
        }
    }
}

/// One row of `projects`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    pub id: String,
    pub kind: ProjectKind,
    pub path: String,
    pub path_key: String,
    pub name: String,
    pub workspaces_root: String,
    pub added_at: i64,
}

const SELECT_PROJECTS: &str =
    "SELECT id, kind, path, path_key, name, workspaces_root, added_at FROM projects";

fn project_from_row(row: &Row<'_>) -> rusqlite::Result<ProjectRow> {
    Ok(ProjectRow {
        id: row.get("id")?,
        kind: row.get("kind")?,
        path: row.get("path")?,
        path_key: row.get("path_key")?,
        name: row.get("name")?,
        workspaces_root: row.get("workspaces_root")?,
        added_at: row.get("added_at")?,
    })
}

impl Store {
    /// Inserts a project. A duplicate id, name or path key is `StoreError::Conflict`.
    pub fn insert_project(&self, row: &ProjectRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO projects (id, kind, path, path_key, name, workspaces_root, added_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                row.id,
                row.kind,
                row.path,
                row.path_key,
                row.name,
                row.workspaces_root,
                row.added_at
            ],
        )?;
        Ok(())
    }

    pub fn get_project(&self, id: &str) -> Result<Option<ProjectRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_PROJECTS} WHERE id = ?1"),
            [id],
            project_from_row,
        )
    }

    pub fn find_project_by_name(&self, name: &str) -> Result<Option<ProjectRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_PROJECTS} WHERE name = ?1"),
            [name],
            project_from_row,
        )
    }

    pub fn find_project_by_path_key(
        &self,
        path_key: &str,
    ) -> Result<Option<ProjectRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_PROJECTS} WHERE path_key = ?1"),
            [path_key],
            project_from_row,
        )
    }

    /// All projects ordered by name.
    pub fn list_projects(&self) -> Result<Vec<ProjectRow>, StoreError> {
        self.query_all(
            &format!("{SELECT_PROJECTS} ORDER BY name"),
            [],
            project_from_row,
        )
    }

    /// Returns whether a row was removed. Fails with `StoreError::Constraint` while workspaces reference the project.
    pub fn delete_project(&self, id: &str) -> Result<bool, StoreError> {
        let removed = self
            .lock()
            .execute("DELETE FROM projects WHERE id = ?1", [id])?;
        Ok(removed > 0)
    }
}
