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
    pub base_ref: Option<String>,
    /// The project's icon as the JSON of a service `ProjectIcon`; `None` is the default icon.
    pub icon: Option<String>,
}

const SELECT_PROJECTS: &str = "SELECT id, kind, path, path_key, name, workspaces_root, added_at, base_ref, icon FROM projects";

fn project_from_row(row: &Row<'_>) -> rusqlite::Result<ProjectRow> {
    Ok(ProjectRow {
        id: row.get("id")?,
        kind: row.get("kind")?,
        path: row.get("path")?,
        path_key: row.get("path_key")?,
        name: row.get("name")?,
        workspaces_root: row.get("workspaces_root")?,
        added_at: row.get("added_at")?,
        base_ref: row.get("base_ref")?,
        icon: row.get("icon")?,
    })
}

impl Store {
    /// Inserts a project. A duplicate id, name or path key is `StoreError::Conflict`.
    pub fn insert_project(&self, row: &ProjectRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO projects (id, kind, path, path_key, name, workspaces_root, added_at, base_ref, icon) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                row.id,
                row.kind,
                row.path,
                row.path_key,
                row.name,
                row.workspaces_root,
                row.added_at,
                row.base_ref,
                row.icon
            ],
        )?;
        Ok(())
    }

    /// Inserts the hidden project of a conversations folder (decision 0094). It is kept out of `list_projects`. A duplicate
    /// id, name or path key is `StoreError::Conflict`.
    pub fn insert_conversations_project(&self, row: &ProjectRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO projects (id, kind, path, path_key, name, workspaces_root, added_at, base_ref, icon, conversations) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1)",
            params![
                row.id,
                row.kind,
                row.path,
                row.path_key,
                row.name,
                row.workspaces_root,
                row.added_at,
                row.base_ref,
                row.icon
            ],
        )?;
        Ok(())
    }

    /// The hidden project of the conversations folder whose path key is `path_key`, if one exists.
    pub fn find_conversations_project(
        &self,
        path_key: &str,
    ) -> Result<Option<ProjectRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_PROJECTS} WHERE path_key = ?1 AND conversations = 1"),
            [path_key],
            project_from_row,
        )
    }

    /// Sets or clears (`None`) a project's base. Returns whether a row was updated.
    pub fn set_project_base(&self, id: &str, base: Option<&str>) -> Result<bool, StoreError> {
        let updated = self.lock().execute(
            "UPDATE projects SET base_ref = ?2 WHERE id = ?1",
            params![id, base],
        )?;
        Ok(updated > 0)
    }

    /// Sets or clears (`None`) a project's icon JSON. Returns whether a row was updated.
    pub fn set_project_icon(&self, id: &str, icon: Option<&str>) -> Result<bool, StoreError> {
        let updated = self.lock().execute(
            "UPDATE projects SET icon = ?2 WHERE id = ?1",
            params![id, icon],
        )?;
        Ok(updated > 0)
    }

    /// Turns the folder project `id` into a git project with `workspaces_root`. Returns whether a folder row was
    /// updated, so a project that is no longer a folder is left alone.
    pub fn set_project_git(&self, id: &str, workspaces_root: &str) -> Result<bool, StoreError> {
        let updated = self.lock().execute(
            "UPDATE projects SET kind = ?2, workspaces_root = ?3 WHERE id = ?1 AND kind = ?4",
            params![id, ProjectKind::Git, workspaces_root, ProjectKind::Folder],
        )?;
        Ok(updated > 0)
    }

    pub fn get_project(&self, id: &str) -> Result<Option<ProjectRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_PROJECTS} WHERE id = ?1"),
            [id],
            project_from_row,
        )
    }

    /// The project with `id` that a person can name, leaving out the hidden projects of conversations folders.
    pub fn get_listed_project(&self, id: &str) -> Result<Option<ProjectRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_PROJECTS} WHERE id = ?1 AND conversations = 0"),
            [id],
            project_from_row,
        )
    }

    /// The project named `name`, leaving out the hidden projects of conversations folders.
    pub fn find_project_by_name(&self, name: &str) -> Result<Option<ProjectRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_PROJECTS} WHERE name = ?1 AND conversations = 0"),
            [name],
            project_from_row,
        )
    }

    /// The project at `path_key`, leaving out the hidden projects of conversations folders.
    pub fn find_project_by_path_key(
        &self,
        path_key: &str,
    ) -> Result<Option<ProjectRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_PROJECTS} WHERE path_key = ?1 AND conversations = 0"),
            [path_key],
            project_from_row,
        )
    }

    /// All projects ordered by name, without the hidden projects of conversations folders.
    pub fn list_projects(&self) -> Result<Vec<ProjectRow>, StoreError> {
        self.query_all(
            &format!("{SELECT_PROJECTS} WHERE conversations = 0 ORDER BY name"),
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

    /// Deletes a project and every workspace row of it in one transaction, returning the workspace ids.
    /// Their agent sessions go with them (`ON DELETE CASCADE`). `None` when no project has `id`.
    pub fn forget_project(&self, id: &str) -> Result<Option<Vec<String>>, StoreError> {
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        let workspaces = {
            let mut statement = transaction.prepare(
                "SELECT id FROM workspaces WHERE project_id = ?1 ORDER BY created_at, id",
            )?;
            let rows = statement.query_map([id], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        transaction.execute("DELETE FROM workspaces WHERE project_id = ?1", [id])?;
        if transaction.execute("DELETE FROM projects WHERE id = ?1", [id])? == 0 {
            return Ok(None);
        }
        transaction.commit()?;
        Ok(Some(workspaces))
    }
}
