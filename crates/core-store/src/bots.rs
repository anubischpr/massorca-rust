use rusqlite::{Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// One row of `bots`. At most one bot per team has `lead` set; the schema enforces it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotRow {
    pub id: String,
    pub team_id: String,
    pub name: String,
    pub job: String,
    pub kind: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub lead: bool,
    pub created_at: i64,
}

const SELECT_BOTS: &str =
    "SELECT id, team_id, name, job, kind, model, effort, lead, created_at FROM bots";

fn bot_from_row(row: &Row<'_>) -> rusqlite::Result<BotRow> {
    Ok(BotRow {
        id: row.get("id")?,
        team_id: row.get("team_id")?,
        name: row.get("name")?,
        job: row.get("job")?,
        kind: row.get("kind")?,
        model: row.get("model")?,
        effort: row.get("effort")?,
        lead: row.get("lead")?,
        created_at: row.get("created_at")?,
    })
}

impl Store {
    /// Inserts a bot. A name taken in its team, or a second lead, is `StoreError::Conflict`.
    pub fn insert_bot(&self, row: &BotRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO bots (id, team_id, name, job, kind, model, effort, lead, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                row.id,
                row.team_id,
                row.name,
                row.job,
                row.kind,
                row.model,
                row.effort,
                row.lead,
                row.created_at
            ],
        )?;
        Ok(())
    }

    pub fn get_bot(&self, id: &str) -> Result<Option<BotRow>, StoreError> {
        self.query_optional(&format!("{SELECT_BOTS} WHERE id = ?1"), [id], bot_from_row)
    }

    pub fn find_bot_by_name(
        &self,
        team_id: &str,
        name: &str,
    ) -> Result<Option<BotRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_BOTS} WHERE team_id = ?1 AND name = ?2"),
            [team_id, name],
            bot_from_row,
        )
    }

    pub fn find_lead_bot(&self, team_id: &str) -> Result<Option<BotRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_BOTS} WHERE team_id = ?1 AND lead = 1"),
            [team_id],
            bot_from_row,
        )
    }

    /// The bots of one team, in creation order.
    pub fn list_bots(&self, team_id: &str) -> Result<Vec<BotRow>, StoreError> {
        self.query_all(
            &format!("{SELECT_BOTS} WHERE team_id = ?1 ORDER BY created_at, id"),
            [team_id],
            bot_from_row,
        )
    }

    /// Changes a bot's name, job, kind, model, effort and lead flag. Returns whether a row changed.
    pub fn update_bot(&self, row: &BotRow) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE bots SET name = ?2, job = ?3, kind = ?4, model = ?5, effort = ?6, lead = ?7 WHERE id = ?1",
            params![
                row.id,
                row.name,
                row.job,
                row.kind,
                row.model,
                row.effort,
                row.lead
            ],
        )?;
        Ok(changed > 0)
    }

    /// Deletes a bot. Returns whether a row was removed. Its tasks keep no bot.
    pub fn delete_bot(&self, id: &str) -> Result<bool, StoreError> {
        let removed = self
            .lock()
            .execute("DELETE FROM bots WHERE id = ?1", [id])?;
        Ok(removed > 0)
    }
}
