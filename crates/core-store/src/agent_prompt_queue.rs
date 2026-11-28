//! Queued prompts (decision 0084): the prompts a person queued for an agent session while its agent was busy. Rows are
//! delivered in position order, one at a time. A row is `waiting` (its `reason` names a pause that holds it, when one
//! does), `sending` (its delivery is being typed) or `failed` (its `reason` says why the delivery failed).

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// Where a queued prompt is in its delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueuedState {
    Waiting,
    Sending,
    Failed,
}

impl QueuedState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Sending => "sending",
            Self::Failed => "failed",
        }
    }
}

impl ToSql for QueuedState {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

impl FromSql for QueuedState {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "waiting" => Ok(Self::Waiting),
            "sending" => Ok(Self::Sending),
            "failed" => Ok(Self::Failed),
            other => Err(FromSqlError::Other(
                format!("unknown queued prompt state {other:?}").into(),
            )),
        }
    }
}

/// One row of `agent_prompt_queue`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedPromptRow {
    pub id: String,
    pub session_id: String,
    /// The row's place in its session's queue, from 1; the lowest is delivered first.
    pub position: i64,
    pub text: String,
    /// The caller's request id, unique within the session when given.
    pub request_id: Option<String>,
    pub state: QueuedState,
    /// For a `waiting` row, the pause that holds it; for a `failed` row, why its delivery failed. Never a prompt's text.
    pub reason: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    /// Whether the caller that queued it held host-admin scope when it did.
    pub host_admin: bool,
    /// The files the prompt sends, as a JSON array of the paths the caller gave (decision 0172). The store keeps the text as
    /// it is; core-agents writes it and reads it, and checks the files again before the prompt is typed.
    pub attachments_json: String,
}

/// A prompt to queue. Its position is the next one of its session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewQueuedPrompt {
    pub id: String,
    pub session_id: String,
    pub text: String,
    pub request_id: Option<String>,
    /// The time it was queued, in milliseconds since the Unix epoch.
    pub at: i64,
    /// Whether the caller held host-admin scope (decision 0071); a run that later holds secrets types only such prompts.
    pub host_admin: bool,
    /// The files the prompt sends, as a JSON array of paths; `[]` for none (decision 0172).
    pub attachments_json: String,
}

const SELECT_QUEUED: &str = "SELECT id, session_id, position, text, request_id, state, reason, created_at_ms, updated_at_ms, host_admin, attachments_json FROM agent_prompt_queue";

fn queued_from_row(row: &Row<'_>) -> rusqlite::Result<QueuedPromptRow> {
    Ok(QueuedPromptRow {
        id: row.get("id")?,
        session_id: row.get("session_id")?,
        position: row.get("position")?,
        text: row.get("text")?,
        request_id: row.get("request_id")?,
        state: row.get("state")?,
        reason: row.get("reason")?,
        created_at_ms: row.get("created_at_ms")?,
        updated_at_ms: row.get("updated_at_ms")?,
        host_admin: row.get("host_admin")?,
        attachments_json: row.get("attachments_json")?,
    })
}

impl Store {
    /// Queues a prompt at the end of its session's queue and returns the row. A request id already queued in the session
    /// is `StoreError::Conflict`; an unknown session is `StoreError::Constraint`.
    pub fn insert_queued_prompt(
        &self,
        new: &NewQueuedPrompt,
    ) -> Result<QueuedPromptRow, StoreError> {
        self.lock().execute(
            "INSERT INTO agent_prompt_queue (id, session_id, position, text, request_id, state, reason, created_at_ms, updated_at_ms, host_admin, attachments_json) SELECT ?1, ?2, COALESCE(MAX(position), 0) + 1, ?3, ?4, 'waiting', NULL, ?5, ?5, ?6, ?7 FROM agent_prompt_queue WHERE session_id = ?2",
            params![new.id, new.session_id, new.text, new.request_id, new.at, new.host_admin, new.attachments_json],
        )?;
        self.get_queued_prompt(&new.id)?
            .ok_or_else(|| StoreError::Constraint {
                constraint: "agent_prompt_queue".to_owned(),
            })
    }

    pub fn get_queued_prompt(&self, id: &str) -> Result<Option<QueuedPromptRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_QUEUED} WHERE id = ?1"),
            [id],
            queued_from_row,
        )
    }

    /// The prompt a session queued with `request_id`, if any.
    pub fn find_queued_prompt_by_request_id(
        &self,
        session_id: &str,
        request_id: &str,
    ) -> Result<Option<QueuedPromptRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_QUEUED} WHERE session_id = ?1 AND request_id = ?2"),
            [session_id, request_id],
            queued_from_row,
        )
    }

    /// A session's queue in position order, the head first.
    pub fn list_queued_prompts(
        &self,
        session_id: &str,
    ) -> Result<Vec<QueuedPromptRow>, StoreError> {
        self.query_all(
            &format!("{SELECT_QUEUED} WHERE session_id = ?1 ORDER BY position"),
            [session_id],
            queued_from_row,
        )
    }

    /// Replaces a prompt's text and records the scope of the caller that made the edit, which is the scope a run that holds
    /// vault secrets is checked against (decision 0084, D7). Returns whether a row matched.
    pub fn update_queued_prompt_text(
        &self,
        id: &str,
        text: &str,
        host_admin: bool,
        at: i64,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE agent_prompt_queue SET text = ?2, host_admin = ?3, updated_at_ms = ?4 WHERE id = ?1",
            params![id, text, host_admin, at],
        )?;
        Ok(changed > 0)
    }

    /// Sets a prompt's state and reason together. Returns whether a row matched.
    pub fn set_queued_prompt_state(
        &self,
        id: &str,
        state: QueuedState,
        reason: Option<&str>,
        at: i64,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE agent_prompt_queue SET state = ?2, reason = ?3, updated_at_ms = ?4 WHERE id = ?1",
            params![id, state, reason, at],
        )?;
        Ok(changed > 0)
    }

    /// Removes a prompt. Returns whether a row was removed.
    pub fn delete_queued_prompt(&self, id: &str) -> Result<bool, StoreError> {
        let removed = self
            .lock()
            .execute("DELETE FROM agent_prompt_queue WHERE id = ?1", [id])?;
        Ok(removed > 0)
    }

    /// Pauses the session's waiting prompts that no pause holds yet, with `reason`. Returns how many changed.
    pub fn pause_queued_prompts(
        &self,
        session_id: &str,
        reason: &str,
        at: i64,
    ) -> Result<u64, StoreError> {
        let changed = self.lock().execute(
            "UPDATE agent_prompt_queue SET reason = ?2, updated_at_ms = ?3 WHERE session_id = ?1 AND state = 'waiting' AND reason IS NULL",
            params![session_id, reason, at],
        )?;
        Ok(u64::try_from(changed).unwrap_or(u64::MAX))
    }

    /// Lifts the pauses on the session's waiting prompts. Returns how many changed.
    pub fn unpause_queued_prompts(&self, session_id: &str, at: i64) -> Result<u64, StoreError> {
        let changed = self.lock().execute(
            "UPDATE agent_prompt_queue SET reason = NULL, updated_at_ms = ?2 WHERE session_id = ?1 AND state = 'waiting' AND reason IN ('paused_stopped', 'paused_restarted')",
            params![session_id, at],
        )?;
        Ok(u64::try_from(changed).unwrap_or(u64::MAX))
    }

    /// The service restarted: every waiting prompt is paused as `paused_restarted`, and a prompt that was being sent
    /// fails, since nothing says whether its text reached the agent. Returns how many prompts changed.
    pub fn restart_queued_prompts(&self, at: i64) -> Result<u64, StoreError> {
        // Both updates in one transaction: a restart never leaves a sending prompt unmarked while the waiting ones are paused.
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        let failed = transaction.execute(
            "UPDATE agent_prompt_queue SET state = 'failed', reason = 'the service restarted while sending', updated_at_ms = ?1 WHERE state = 'sending'",
            [at],
        )?;
        let paused = transaction.execute(
            "UPDATE agent_prompt_queue SET reason = 'paused_restarted', updated_at_ms = ?1 WHERE state = 'waiting'",
            [at],
        )?;
        transaction.commit()?;
        Ok(u64::try_from(failed + paused).unwrap_or(u64::MAX))
    }

    /// Records that a session delivered the prompt queued with `request_id` at `at`, keeping only the `keep` most recent
    /// ids of the session (decision 0084). A repeated id keeps its first time.
    pub fn record_delivered_request(
        &self,
        session_id: &str,
        request_id: &str,
        at: i64,
        keep: u32,
    ) -> Result<(), StoreError> {
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT OR IGNORE INTO agent_prompt_delivered (session_id, request_id, delivered_at_ms) VALUES (?1, ?2, ?3)",
            params![session_id, request_id, at],
        )?;
        transaction.execute(
            "DELETE FROM agent_prompt_delivered WHERE session_id = ?1 AND rowid NOT IN (SELECT rowid FROM agent_prompt_delivered WHERE session_id = ?1 ORDER BY rowid DESC LIMIT ?2)",
            params![session_id, keep],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// When a session delivered the prompt queued with `request_id`, if it still remembers it.
    pub fn find_delivered_request(
        &self,
        session_id: &str,
        request_id: &str,
    ) -> Result<Option<i64>, StoreError> {
        self.query_optional(
            "SELECT delivered_at_ms FROM agent_prompt_delivered WHERE session_id = ?1 AND request_id = ?2",
            [session_id, request_id],
            |row| row.get(0),
        )
    }
}
