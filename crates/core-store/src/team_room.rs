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
    /// The one-time routine the line scheduled (decision 0121).
    pub routine_id: Option<String>,
    /// A choice card's options, as the service encodes them (decision 0121); `None` for any other line.
    pub card: Option<String>,
    /// The choice a card was answered with; set once.
    pub card_answer: Option<String>,
    /// The line this one replies to (decision 0122).
    pub reply_to: Option<String>,
    /// The bot that answers a person's conversation line.
    pub reply_bot_id: Option<String>,
    /// The agent session of that bot's reply run.
    pub reply_session: Option<String>,
    /// A conversation line's reply: `open`, `answered` (the bot posted, its run still ends) or `closed`.
    pub reply_state: Option<String>,
    pub created_at: i64,
}

const SELECT_ROOM: &str = "SELECT id, team_id, author, bot_id, text, task_id, routine_id, card, card_answer, reply_to, reply_bot_id, reply_session, reply_state, created_at FROM team_room_messages";

fn room_from_row(row: &Row<'_>) -> rusqlite::Result<TeamRoomMessageRow> {
    Ok(TeamRoomMessageRow {
        id: row.get("id")?,
        team_id: row.get("team_id")?,
        author: row.get("author")?,
        bot_id: row.get("bot_id")?,
        text: row.get("text")?,
        task_id: row.get("task_id")?,
        routine_id: row.get("routine_id")?,
        card: row.get("card")?,
        card_answer: row.get("card_answer")?,
        reply_to: row.get("reply_to")?,
        reply_bot_id: row.get("reply_bot_id")?,
        reply_session: row.get("reply_session")?,
        reply_state: row.get("reply_state")?,
        created_at: row.get("created_at")?,
    })
}

impl Store {
    /// Inserts a room line. An unknown team or bot is `StoreError::Constraint`.
    pub fn insert_team_room_message(&self, row: &TeamRoomMessageRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO team_room_messages (id, team_id, author, bot_id, text, task_id, routine_id, card, card_answer, reply_to, reply_bot_id, reply_session, reply_state, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                row.id,
                row.team_id,
                row.author,
                row.bot_id,
                row.text,
                row.task_id,
                row.routine_id,
                row.card,
                row.card_answer,
                row.reply_to,
                row.reply_bot_id,
                row.reply_session,
                row.reply_state,
                row.created_at
            ],
        )?;
        Ok(())
    }

    /// One room line by id.
    pub fn get_team_room_message(
        &self,
        id: &str,
    ) -> Result<Option<TeamRoomMessageRow>, StoreError> {
        self.query_optional(&format!("{SELECT_ROOM} WHERE id = ?1"), [id], room_from_row)
    }

    /// Records `answer` on the choice card `id` unless it was answered: `false` for an answered card or a line that is
    /// not a card. One statement, so two answers racing record one.
    pub fn answer_team_room_card(&self, id: &str, answer: &str) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE team_room_messages SET card_answer = ?2 WHERE id = ?1 AND card IS NOT NULL AND card_answer IS NULL",
            params![id, answer],
        )?;
        Ok(changed == 1)
    }

    /// Clears a card's answer, when what the answer was to make could not be made.
    pub fn release_team_room_card(&self, id: &str) -> Result<(), StoreError> {
        self.lock().execute(
            "UPDATE team_room_messages SET card_answer = NULL WHERE id = ?1",
            [id],
        )?;
        Ok(())
    }

    /// Names the reply run of the conversation line `id`.
    pub fn set_team_room_reply_session(&self, id: &str, session: &str) -> Result<(), StoreError> {
        self.lock().execute(
            "UPDATE team_room_messages SET reply_session = ?2 WHERE id = ?1",
            params![id, session],
        )?;
        Ok(())
    }

    /// The conversation line whose reply run is `session`.
    pub fn find_team_room_reply(
        &self,
        session: &str,
    ) -> Result<Option<TeamRoomMessageRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_ROOM} WHERE reply_session = ?1"),
            [session],
            room_from_row,
        )
    }

    /// Moves the reply of line `id` from `from` to `to`: `false` when it was not in `from`. One statement, so two
    /// answers racing move it once.
    pub fn move_team_room_reply(&self, id: &str, from: &str, to: &str) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE team_room_messages SET reply_state = ?3 WHERE id = ?1 AND reply_state = ?2",
            params![id, from, to],
        )?;
        Ok(changed == 1)
    }

    /// A team's conversation lines whose reply is open, oldest first.
    pub fn open_team_room_replies(
        &self,
        team_id: &str,
    ) -> Result<Vec<TeamRoomMessageRow>, StoreError> {
        self.query_all(
            &format!(
                "{SELECT_ROOM} WHERE team_id = ?1 AND reply_state = 'open' ORDER BY created_at, rowid"
            ),
            [team_id],
            room_from_row,
        )
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
