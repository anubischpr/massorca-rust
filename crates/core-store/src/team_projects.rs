use rusqlite::{Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// One row of `team_projects`: a project of a team, with its own Board and Brief.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamProjectRow {
    pub id: String,
    pub team_id: String,
    pub name: String,
    pub goal: String,
    pub brief: String,
    pub created_at: i64,
    /// The Workspaces project the code comes from; cleared when that project is removed.
    pub source_project_id: Option<String>,
}

const SELECT_TEAM_PROJECTS: &str =
    "SELECT id, team_id, name, goal, brief, created_at, source_project_id FROM team_projects";

fn team_project_from_row(row: &Row<'_>) -> rusqlite::Result<TeamProjectRow> {
    Ok(TeamProjectRow {
        id: row.get("id")?,
        team_id: row.get("team_id")?,
        name: row.get("name")?,
        goal: row.get("goal")?,
        brief: row.get("brief")?,
        created_at: row.get("created_at")?,
        source_project_id: row.get("source_project_id")?,
    })
}

impl Store {
    /// Inserts a team project. A duplicate id, or a name taken in its team, is `StoreError::Conflict`.
    pub fn insert_team_project(&self, row: &TeamProjectRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO team_projects (id, team_id, name, goal, brief, created_at, source_project_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                row.id,
                row.team_id,
                row.name,
                row.goal,
                row.brief,
                row.created_at,
                row.source_project_id
            ],
        )?;
        Ok(())
    }

    pub fn get_team_project(&self, id: &str) -> Result<Option<TeamProjectRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_TEAM_PROJECTS} WHERE id = ?1"),
            [id],
            team_project_from_row,
        )
    }

    pub fn find_team_project_by_name(
        &self,
        team_id: &str,
        name: &str,
    ) -> Result<Option<TeamProjectRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_TEAM_PROJECTS} WHERE team_id = ?1 AND name = ?2"),
            [team_id, name],
            team_project_from_row,
        )
    }

    /// The projects of one team, in creation order.
    pub fn list_team_projects(&self, team_id: &str) -> Result<Vec<TeamProjectRow>, StoreError> {
        self.query_all(
            &format!("{SELECT_TEAM_PROJECTS} WHERE team_id = ?1 ORDER BY created_at, id"),
            [team_id],
            team_project_from_row,
        )
    }

    /// Changes a project's name, goal, brief and source project. Returns whether a row changed.
    pub fn update_team_project(&self, row: &TeamProjectRow) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE team_projects SET name = ?2, goal = ?3, brief = ?4, source_project_id = ?5 WHERE id = ?1",
            params![row.id, row.name, row.goal, row.brief, row.source_project_id],
        )?;
        Ok(changed > 0)
    }

    /// Deletes a project. Returns whether a row was removed. Its tasks keep no project.
    pub fn delete_team_project(&self, id: &str) -> Result<bool, StoreError> {
        let removed = self
            .lock()
            .execute("DELETE FROM team_projects WHERE id = ?1", [id])?;
        Ok(removed > 0)
    }
}
