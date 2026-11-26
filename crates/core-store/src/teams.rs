use rusqlite::{Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// One row of `teams`. `character` and `access` hold the protocol's names; the service validates them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamRow {
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub character: String,
    pub access: String,
    pub workspaces: bool,
    pub created_at: i64,
}

const SELECT_TEAMS: &str =
    "SELECT id, name, purpose, character, access, workspaces, created_at FROM teams";

fn team_from_row(row: &Row<'_>) -> rusqlite::Result<TeamRow> {
    Ok(TeamRow {
        id: row.get("id")?,
        name: row.get("name")?,
        purpose: row.get("purpose")?,
        character: row.get("character")?,
        access: row.get("access")?,
        workspaces: row.get("workspaces")?,
        created_at: row.get("created_at")?,
    })
}

impl Store {
    /// Inserts a team. A duplicate id or name is `StoreError::Conflict`.
    pub fn insert_team(&self, row: &TeamRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO teams (id, name, purpose, character, access, workspaces, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                row.id,
                row.name,
                row.purpose,
                row.character,
                row.access,
                row.workspaces,
                row.created_at
            ],
        )?;
        Ok(())
    }

    pub fn get_team(&self, id: &str) -> Result<Option<TeamRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_TEAMS} WHERE id = ?1"),
            [id],
            team_from_row,
        )
    }

    pub fn find_team_by_name(&self, name: &str) -> Result<Option<TeamRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_TEAMS} WHERE name = ?1"),
            [name],
            team_from_row,
        )
    }

    /// All teams ordered by name.
    pub fn list_teams(&self) -> Result<Vec<TeamRow>, StoreError> {
        self.query_all(&format!("{SELECT_TEAMS} ORDER BY name"), [], team_from_row)
    }

    /// Changes a team's name, purpose, character, access and workspaces flag. Returns whether a row changed.
    pub fn update_team(&self, row: &TeamRow) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE teams SET name = ?2, purpose = ?3, character = ?4, access = ?5, workspaces = ?6 WHERE id = ?1",
            params![
                row.id,
                row.name,
                row.purpose,
                row.character,
                row.access,
                row.workspaces
            ],
        )?;
        Ok(changed > 0)
    }

    /// The team's folder workspace, once its first task started. `None` for an unknown team.
    pub fn team_workspace(&self, team_id: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .query_optional(
                "SELECT workspace_id FROM teams WHERE id = ?1",
                [team_id],
                |row| row.get::<_, Option<String>>("workspace_id"),
            )?
            .flatten())
    }

    /// Records the team's folder workspace. Returns whether the team exists.
    pub fn set_team_workspace(
        &self,
        team_id: &str,
        workspace_id: &str,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE teams SET workspace_id = ?2 WHERE id = ?1",
            params![team_id, workspace_id],
        )?;
        Ok(changed > 0)
    }

    /// Deletes a team with its bots, projects and tasks. Returns whether a row was removed.
    pub fn delete_team(&self, id: &str) -> Result<bool, StoreError> {
        let removed = self
            .lock()
            .execute("DELETE FROM teams WHERE id = ?1", [id])?;
        Ok(removed > 0)
    }
}

impl Store {
    /// Links `child` to the `parent` task that delegated it (decision 0128). A child has one parent.
    pub fn insert_task_delegation(
        &self,
        child: &str,
        parent: &str,
        read_only: bool,
        now: i64,
    ) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO task_delegations (child_task_id, parent_task_id, read_only, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![child, parent, read_only, now],
        )?;
        Ok(())
    }

    /// The parent task of `child` and whether the child only reads in it; `None` for a task nobody delegated.
    pub fn task_delegation_parent(
        &self,
        child: &str,
    ) -> Result<Option<(String, bool)>, StoreError> {
        self.query_optional(
            "SELECT parent_task_id, read_only FROM task_delegations WHERE child_task_id = ?1",
            [child],
            |row| Ok((row.get("parent_task_id")?, row.get("read_only")?)),
        )
    }

    /// The child tasks `parent` delegated, oldest first.
    pub fn task_delegation_children(&self, parent: &str) -> Result<Vec<String>, StoreError> {
        self.query_all(
            "SELECT child_task_id FROM task_delegations WHERE parent_task_id = ?1 ORDER BY created_at, child_task_id",
            [parent],
            |row| row.get("child_task_id"),
        )
    }
}
