use rusqlite::{Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// One line of a team's Conference room (decision 0102). `author` is `person` or `bot`; a bot line names its bot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamRoomMessageRow {
    pub id: String,
    pub team_id: String,
    pub author: String,
    pub bot_id: Option<String>,
    pub text: String,
    /// The task the line is about, for its Board card.
    pub task_id: Option<String>,
    pub created_at: i64,
}

const SELECT_ROOM: &str =
    "SELECT id, team_id, author, bot_id, text, task_id, created_at FROM team_room_messages";

fn room_from_row(row: &Row<'_>) -> rusqlite::Result<TeamRoomMessageRow> {
    Ok(TeamRoomMessageRow {
        id: row.get("id")?,
        team_id: row.get("team_id")?,
        author: row.get("author")?,
        bot_id: row.get("bot_id")?,
        text: row.get("text")?,
        task_id: row.get("task_id")?,
        created_at: row.get("created_at")?,
    })
}

impl Store {
    /// Inserts a room line. An unknown team or bot is `StoreError::Constraint`.
    pub fn insert_team_room_message(&self, row: &TeamRoomMessageRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO team_room_messages (id, team_id, author, bot_id, text, task_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                row.id,
                row.team_id,
                row.author,
                row.bot_id,
                row.text,
                row.task_id,
                row.created_at
            ],
        )?;
        Ok(())
    }

    /// A team's room, oldest first. `limit` keeps the newest that many.
    pub fn list_team_room(
        &self,
        team_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<TeamRoomMessageRow>, StoreError> {
        let limit = limit.map_or_else(String::new, |count| format!(" LIMIT {count}"));
        let mut rows = self.query_all(
            &format!(
                "{SELECT_ROOM} WHERE team_id = ?1 ORDER BY created_at DESC, rowid DESC{limit}"
            ),
            [team_id],
            room_from_row,
        )?;
        rows.reverse();
        Ok(rows)
    }
}
