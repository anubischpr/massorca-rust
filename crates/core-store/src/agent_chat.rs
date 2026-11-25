//! Structured chat runs and their transcript journal (decision 0101): how a session's run talks to the service, and per
//! session the journal's epoch, its newest change and its items. The store knows items as rows with a JSON body; the
//! item vocabulary is the protocol's, read and written by `core-agents`.

use rusqlite::{OptionalExtension, Row, Transaction, params};

use crate::agent_sessions::{AgentSessionRow, RunExit};
use crate::error::StoreError;
use crate::store::Store;

/// How a session's run talks to the service.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SessionRunType {
    /// The vendor CLI runs in a terminal (decision 0051).
    #[default]
    Terminal,
    /// The vendor CLI speaks a structured protocol over pipes (decision 0101).
    Chat,
}

impl SessionRunType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::Chat => "chat",
        }
    }

    fn parse(text: &str) -> Self {
        // The table's check allows only these two.
        if text == "chat" {
            Self::Chat
        } else {
            Self::Terminal
        }
    }
}

/// A session's run type, and the id of its chat run (the part a terminal id plays for a terminal run).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionRun {
    pub run_type: SessionRunType,
    pub run_id: Option<String>,
}

/// One session's journal: its epoch, the `seq` of its newest change, and the newest `seq` of an item it dropped to keep
/// its cap (0 when it dropped none). A cursor older than `pruned_seq` can no longer be replayed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatJournalHead {
    pub epoch: String,
    pub seq: u64,
    pub pruned_seq: u64,
}

/// One journal item as stored. `body` is the item's JSON body; `text` is kept apart for an item whose text grows by
/// appends (it is `None` for every other item): its base text with every appended chunk after it, in byte order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatItemRow {
    pub id: String,
    pub position: u64,
    pub seq: u64,
    pub turn: Option<String>,
    pub parent: Option<String>,
    pub at_ms: i64,
    pub item_type: String,
    pub body: String,
    pub text: Option<String>,
}

/// An item to add; it gets the next `seq` as both its position and its last change.
#[derive(Debug, Clone, Copy)]
pub struct NewChatItem<'a> {
    pub id: &'a str,
    pub turn: Option<&'a str>,
    pub parent: Option<&'a str>,
    pub at_ms: i64,
    pub item_type: &'a str,
    pub body: &'a str,
    /// Set for an item whose text grows by appends.
    pub text: Option<&'a str>,
}

/// An append that was applied: its change `seq`, and the byte length of the text before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatAppend {
    pub seq: u64,
    pub offset: u64,
}

/// The items with their text whole: the base, then each chunk in byte order.
const SELECT_ITEMS: &str = "SELECT id, position, seq, turn_id, parent_id, at_ms, type, body, CASE WHEN text IS NULL THEN NULL ELSE text || COALESCE((SELECT group_concat(chunk.text, '' ORDER BY chunk.start_byte) FROM agent_chat_text_chunks AS chunk WHERE chunk.session_id = item.session_id AND chunk.item_id = item.id), '') END FROM agent_chat_items AS item";

fn item_from_row(row: &Row<'_>) -> rusqlite::Result<ChatItemRow> {
    Ok(ChatItemRow {
        id: row.get(0)?,
        position: to_u64(row.get(1)?),
        seq: to_u64(row.get(2)?),
        turn: row.get(3)?,
        parent: row.get(4)?,
        at_ms: row.get(5)?,
        item_type: row.get(6)?,
        body: row.get(7)?,
        text: row.get(8)?,
    })
}

/// The byte length of `text` as SQLite counts it.
fn byte_len(text: &str) -> i64 {
    i64::try_from(text.len()).unwrap_or(i64::MAX)
}

fn to_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Takes the journal's next `seq` inside `transaction`; `None` when the session has no journal.
fn next_seq(transaction: &Transaction<'_>, session: &str) -> rusqlite::Result<Option<u64>> {
    let changed = transaction.execute(
        "UPDATE agent_chat_journals SET seq = seq + 1 WHERE session_id = ?1",
        [session],
    )?;
    if changed == 0 {
        return Ok(None);
    }
    let seq: i64 = transaction.query_row(
        "SELECT seq FROM agent_chat_journals WHERE session_id = ?1",
        [session],
        |row| row.get(0),
    )?;
    Ok(Some(to_u64(seq)))
}

impl Store {
    /// Inserts a session whose run is a chat run with id `run_id`, in one transaction with its row.
    pub fn insert_chat_agent_session(
        &self,
        row: &AgentSessionRow,
        run_id: &str,
    ) -> Result<(), StoreError> {
        let connection = self.lock();
        let transaction = connection.unchecked_transaction()?;
        Self::insert_agent_session_in(&transaction, row)?;
        transaction.execute(
            "UPDATE agent_sessions SET run_type = 'chat', run_id = ?2 WHERE id = ?1",
            params![row.id, run_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// The run type of a session and its chat run id; `None` when there is no such session.
    pub fn agent_session_run(&self, id: &str) -> Result<Option<SessionRun>, StoreError> {
        self.query_optional(
            "SELECT run_type, run_id FROM agent_sessions WHERE id = ?1",
            [id],
            |row| {
                Ok(SessionRun {
                    run_type: SessionRunType::parse(&row.get::<_, String>(0)?),
                    run_id: row.get(1)?,
                })
            },
        )
    }

    /// Records how a chat run ended, unless an end is already recorded, and only while the row still names `run_id`: the
    /// chat form of [`Store::set_agent_session_exit`] (decision 0058). Returns whether the row changed.
    pub fn set_agent_session_chat_exit(
        &self,
        id: &str,
        run_id: &str,
        exit: &RunExit,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE agent_sessions SET exit_code = ?3, exit_signal = ?4, exit_reason = ?5, exited_at = ?6 WHERE id = ?1 AND run_type = 'chat' AND run_id = ?2 AND exit_reason IS NULL",
            params![id, run_id, exit.code, exit.signal, exit.reason, exit.at],
        )?;
        Ok(changed > 0)
    }

    /// Opens a session's journal with `epoch` when it has none, and returns its head.
    pub fn open_chat_journal(
        &self,
        session: &str,
        epoch: &str,
    ) -> Result<ChatJournalHead, StoreError> {
        let connection = self.lock();
        connection.execute(
            "INSERT OR IGNORE INTO agent_chat_journals (session_id, epoch, seq, pruned_seq) VALUES (?1, ?2, 0, 0)",
            params![session, epoch],
        )?;
        Ok(connection.query_row(
            "SELECT epoch, seq, pruned_seq FROM agent_chat_journals WHERE session_id = ?1",
            [session],
            head_from_row,
        )?)
    }

    /// A session's journal head; `None` when it has no journal.
    pub fn chat_journal_head(&self, session: &str) -> Result<Option<ChatJournalHead>, StoreError> {
        self.query_optional(
            "SELECT epoch, seq, pruned_seq FROM agent_chat_journals WHERE session_id = ?1",
            [session],
            head_from_row,
        )
    }

    /// Adds an item at the journal's next `seq`, which is its position. When the journal then holds more than
    /// `max_items`, the oldest are dropped and `pruned_seq` rises to the newest change among them. Returns the item, or
    /// `None` when the session has no journal.
    pub fn add_chat_item(
        &self,
        session: &str,
        item: &NewChatItem<'_>,
        max_items: u64,
    ) -> Result<Option<ChatItemRow>, StoreError> {
        let connection = self.lock();
        let transaction = connection.unchecked_transaction()?;
        let Some(seq) = next_seq(&transaction, session)? else {
            return Ok(None);
        };
        transaction.execute(
            "INSERT INTO agent_chat_items (session_id, id, position, seq, turn_id, parent_id, at_ms, type, body, text, text_len) VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                session,
                item.id,
                to_i64(seq),
                item.turn,
                item.parent,
                item.at_ms,
                item.item_type,
                item.body,
                item.text,
                item.text.map_or(0, byte_len)
            ],
        )?;
        let cutoff: Option<i64> = transaction
            .query_row(
                "SELECT position FROM agent_chat_items WHERE session_id = ?1 ORDER BY position DESC LIMIT 1 OFFSET ?2",
                params![session, to_i64(max_items)],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(cutoff) = cutoff {
            transaction.execute(
                "UPDATE agent_chat_journals SET pruned_seq = MAX(pruned_seq, (SELECT MAX(seq) FROM agent_chat_items WHERE session_id = ?1 AND position <= ?2)) WHERE session_id = ?1",
                params![session, cutoff],
            )?;
            transaction.execute(
                "DELETE FROM agent_chat_items WHERE session_id = ?1 AND position <= ?2",
                params![session, cutoff],
            )?;
        }
        let row = transaction.query_row(
            &format!("{SELECT_ITEMS} WHERE session_id = ?1 AND id = ?2"),
            params![session, item.id],
            item_from_row,
        )?;
        transaction.commit()?;
        Ok(Some(row))
    }

    /// Replaces an item's body at the journal's next `seq`, and its text when `text` is given. Its position stays. Returns
    /// the item after the change, or `None` when the journal has no such item.
    pub fn replace_chat_item(
        &self,
        session: &str,
        id: &str,
        body: &str,
        text: Option<&str>,
    ) -> Result<Option<ChatItemRow>, StoreError> {
        let connection = self.lock();
        let transaction = connection.unchecked_transaction()?;
        let exists: Option<i64> = transaction
            .query_row(
                "SELECT 1 FROM agent_chat_items WHERE session_id = ?1 AND id = ?2",
                params![session, id],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Ok(None);
        }
        let Some(seq) = next_seq(&transaction, session)? else {
            return Ok(None);
        };
        transaction.execute(
            "UPDATE agent_chat_items SET seq = ?3, body = ?4 WHERE session_id = ?1 AND id = ?2",
            params![session, id, to_i64(seq), body],
        )?;
        // A new text replaces the base and folds every chunk into it.
        if let Some(text) = text {
            transaction.execute(
                "UPDATE agent_chat_items SET text = ?3, text_len = ?4 WHERE session_id = ?1 AND id = ?2",
                params![session, id, text, byte_len(text)],
            )?;
            transaction.execute(
                "DELETE FROM agent_chat_text_chunks WHERE session_id = ?1 AND item_id = ?2",
                params![session, id],
            )?;
        }
        let row = transaction.query_row(
            &format!("{SELECT_ITEMS} WHERE session_id = ?1 AND id = ?2"),
            params![session, id],
            item_from_row,
        )?;
        transaction.commit()?;
        Ok(Some(row))
    }

    /// Appends `text` to the text of an item that keeps one, at the journal's next `seq`, as a chunk of its own: neither
    /// the body nor the text before it is rewritten.
    /// Returns the change and the byte length of the text before it, or `None` when the journal has no such item or the
    /// item keeps no text.
    pub fn append_chat_item_text(
        &self,
        session: &str,
        id: &str,
        text: &str,
    ) -> Result<Option<ChatAppend>, StoreError> {
        let connection = self.lock();
        let transaction = connection.unchecked_transaction()?;
        let length: Option<i64> = transaction
            .query_row(
                "SELECT text_len FROM agent_chat_items WHERE session_id = ?1 AND id = ?2 AND text IS NOT NULL",
                params![session, id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(length) = length else {
            return Ok(None);
        };
        let Some(seq) = next_seq(&transaction, session)? else {
            return Ok(None);
        };
        if !text.is_empty() {
            transaction.execute(
                "INSERT INTO agent_chat_text_chunks (session_id, item_id, start_byte, text) VALUES (?1, ?2, ?3, ?4)",
                params![session, id, length, text],
            )?;
        }
        transaction.execute(
            "UPDATE agent_chat_items SET seq = ?3, text_len = text_len + ?4 WHERE session_id = ?1 AND id = ?2",
            params![session, id, to_i64(seq), byte_len(text)],
        )?;
        transaction.commit()?;
        Ok(Some(ChatAppend {
            seq,
            offset: to_u64(length),
        }))
    }

    /// One item of a session's journal.
    pub fn chat_item(&self, session: &str, id: &str) -> Result<Option<ChatItemRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_ITEMS} WHERE session_id = ?1 AND id = ?2"),
            params![session, id],
            item_from_row,
        )
    }

    /// The newest `limit` items before `before` (a position; every item when `None`), oldest first, and whether older
    /// items exist.
    pub fn chat_items_before(
        &self,
        session: &str,
        before: Option<u64>,
        limit: u32,
    ) -> Result<(Vec<ChatItemRow>, bool), StoreError> {
        let before = before.map_or(i64::MAX, to_i64);
        let mut items = self.query_all(
            &format!(
                "{SELECT_ITEMS} WHERE session_id = ?1 AND position < ?2 ORDER BY position DESC LIMIT ?3"
            ),
            params![session, before, i64::from(limit) + 1],
            item_from_row,
        )?;
        let has_older = items.len() > limit as usize;
        items.truncate(limit as usize);
        items.reverse();
        Ok((items, has_older))
    }

    /// The items changed after `seq`, in the order of their last change, at most `limit` of them.
    pub fn chat_items_changed_after(
        &self,
        session: &str,
        seq: u64,
        limit: u32,
    ) -> Result<Vec<ChatItemRow>, StoreError> {
        self.query_all(
            &format!("{SELECT_ITEMS} WHERE session_id = ?1 AND seq > ?2 ORDER BY seq LIMIT ?3"),
            params![session, to_i64(seq), i64::from(limit)],
            item_from_row,
        )
    }

    /// The items of one turn whose body is of `item_type`, oldest first.
    pub fn chat_turn_items(
        &self,
        session: &str,
        turn: &str,
        item_type: &str,
    ) -> Result<Vec<ChatItemRow>, StoreError> {
        self.query_all(
            &format!(
                "{SELECT_ITEMS} WHERE session_id = ?1 AND turn_id = ?2 AND type = ?3 ORDER BY position"
            ),
            params![session, turn, item_type],
            item_from_row,
        )
    }

    /// The newest item of `item_type` in a session's journal.
    pub fn latest_chat_item_of_type(
        &self,
        session: &str,
        item_type: &str,
    ) -> Result<Option<ChatItemRow>, StoreError> {
        self.query_optional(
            &format!(
                "{SELECT_ITEMS} WHERE session_id = ?1 AND type = ?2 ORDER BY position DESC LIMIT 1"
            ),
            params![session, item_type],
            item_from_row,
        )
    }
}

fn head_from_row(row: &Row<'_>) -> rusqlite::Result<ChatJournalHead> {
    Ok(ChatJournalHead {
        epoch: row.get(0)?,
        seq: to_u64(row.get(1)?),
        pruned_seq: to_u64(row.get(2)?),
    })
}

