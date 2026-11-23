use rusqlite::{Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// Lines a task start reads from each scope: the team's and the bot's.
const START_LIMIT: usize = 40;
const NEWEST_FIRST: &str = "ORDER BY created_at DESC, id DESC";

/// One row of `team_memory`: a line a team keeps, or one of its bots. `source` is `task` or `person`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamMemoryRow {
    pub id: String,
    pub team_id: String,
    /// The bot whose scope the line is in; `None` for the team scope.
    pub bot_id: Option<String>,
    pub text: String,
    pub source: String,
    pub task_id: Option<String>,
    pub created_at: i64,
}

const SELECT_TEAM_MEMORY: &str =
    "SELECT id, team_id, bot_id, text, source, task_id, created_at FROM team_memory";

fn memory_from_row(row: &Row<'_>) -> rusqlite::Result<TeamMemoryRow> {
    Ok(TeamMemoryRow {
        id: row.get("id")?,
        team_id: row.get("team_id")?,
        bot_id: row.get("bot_id")?,
        text: row.get("text")?,
        source: row.get("source")?,
        task_id: row.get("task_id")?,
        created_at: row.get("created_at")?,
    })
}

impl Store {
    /// Inserts a memory line. An unknown team or bot is `StoreError::Constraint`.
    pub fn insert_team_memory(&self, row: &TeamMemoryRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO team_memory (id, team_id, bot_id, text, source, task_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                row.id,
                row.team_id,
                row.bot_id,
                row.text,
                row.source,
                row.task_id,
                row.created_at
            ],
        )?;
        Ok(())
    }

    pub fn get_team_memory(&self, id: &str) -> Result<Option<TeamMemoryRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_TEAM_MEMORY} WHERE id = ?1"),
            [id],
            memory_from_row,
        )
    }

    /// The lines of a team, or only of `bot_id` when given, newest first. `limit` keeps the newest that many.
    pub fn list_team_memory(
        &self,
        team_id: &str,
        bot_id: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<TeamMemoryRow>, StoreError> {
        let limit = limit.map_or_else(String::new, |count| format!(" LIMIT {count}"));
        match bot_id {
            Some(bot_id) => self.query_all(
                &format!(
                    "{SELECT_TEAM_MEMORY} WHERE team_id = ?1 AND bot_id = ?2 {NEWEST_FIRST}{limit}"
                ),
                [team_id, bot_id],
                memory_from_row,
            ),
            None => self.query_all(
                &format!("{SELECT_TEAM_MEMORY} WHERE team_id = ?1 {NEWEST_FIRST}{limit}"),
                [team_id],
                memory_from_row,
            ),
        }
    }

    /// What a task start reads: the newest 40 team-scope lines, then the newest 40 lines of `bot_id`, each newest first.
    pub fn memory_for_start(
        &self,
        team_id: &str,
        bot_id: &str,
    ) -> Result<(Vec<TeamMemoryRow>, Vec<TeamMemoryRow>), StoreError> {
        let team = self.query_all(
            &format!(
                "{SELECT_TEAM_MEMORY} WHERE team_id = ?1 AND bot_id IS NULL {NEWEST_FIRST} LIMIT {START_LIMIT}"
            ),
            [team_id],
            memory_from_row,
        )?;
        let bot = self.query_all(
            &format!(
                "{SELECT_TEAM_MEMORY} WHERE team_id = ?1 AND bot_id = ?2 {NEWEST_FIRST} LIMIT {START_LIMIT}"
            ),
            [team_id, bot_id],
            memory_from_row,
        )?;
        Ok((team, bot))
    }

    /// Deletes a line. Returns whether a row was removed.
    pub fn delete_team_memory(&self, id: &str) -> Result<bool, StoreError> {
        let removed = self
            .lock()
            .execute("DELETE FROM team_memory WHERE id = ?1", [id])?;
        Ok(removed > 0)
    }
}
