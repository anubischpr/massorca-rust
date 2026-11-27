use rusqlite::{OptionalExtension, Row, params};

use crate::error::StoreError;
use crate::store::Store;
use crate::tasks::TaskOutcome;

/// One message between two bots of a delegation (decision 0129). `kind`, `priority`, `outcome` and `delivery` hold the
/// protocol's names; the service validates them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamMessageRow {
    pub id: String,
    pub from_task_id: String,
    pub to_task_id: String,
    pub from_bot_id: Option<String>,
    pub to_bot_id: Option<String>,
    pub kind: String,
    pub priority: String,
    pub subject: String,
    pub body: String,
    pub thread_id: String,
    pub reply_to: Option<String>,
    pub outcome: Option<String>,
    /// The task a `worker_done` settled; one per task and sending run.
    pub settles_task_id: Option<String>,
    /// The agent session of the run that sent it: a reopened task's new run is a new settlement and a new report.
    pub from_session_id: Option<String>,
    /// `inbox`, or `run` once it was handed to the recipient's live run.
    pub delivery: String,
    pub read_at: Option<i64>,
    pub created_at: i64,
}

/// What storing a `worker_done` did (decision 0129).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerDoneStored {
    /// The message is stored and the task's outcome latched with it.
    Settled,
    /// The task had already ended with the same outcome: the message is stored, the outcome is unchanged.
    Matched,
    /// A `worker_done` for the task was already stored: nothing is written, and this is the first one.
    Duplicate(Box<TeamMessageRow>),
    /// The task ended with another outcome: nothing is written.
    Conflict(Option<TaskOutcome>),
}

const SELECT_MESSAGE: &str = "SELECT id, from_task_id, to_task_id, from_bot_id, to_bot_id, kind, priority, subject, body, thread_id, reply_to, outcome, settles_task_id, from_session_id, delivery, read_at, created_at FROM team_messages";

const INSERT_MESSAGE: &str = "INSERT INTO team_messages (id, from_task_id, to_task_id, from_bot_id, to_bot_id, kind, priority, subject, body, thread_id, reply_to, outcome, settles_task_id, from_session_id, delivery, read_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)";

fn message_from_row(row: &Row<'_>) -> rusqlite::Result<TeamMessageRow> {
    Ok(TeamMessageRow {
        id: row.get("id")?,
        from_task_id: row.get("from_task_id")?,
        to_task_id: row.get("to_task_id")?,
        from_bot_id: row.get("from_bot_id")?,
        to_bot_id: row.get("to_bot_id")?,
        kind: row.get("kind")?,
        priority: row.get("priority")?,
        subject: row.get("subject")?,
        body: row.get("body")?,
        thread_id: row.get("thread_id")?,
        reply_to: row.get("reply_to")?,
        outcome: row.get("outcome")?,
        settles_task_id: row.get("settles_task_id")?,
        from_session_id: row.get("from_session_id")?,
        delivery: row.get("delivery")?,
        read_at: row.get("read_at")?,
        created_at: row.get("created_at")?,
    })
}

fn insert(connection: &rusqlite::Connection, row: &TeamMessageRow) -> rusqlite::Result<usize> {
    connection.execute(
        INSERT_MESSAGE,
        params![
            row.id,
            row.from_task_id,
            row.to_task_id,
            row.from_bot_id,
            row.to_bot_id,
            row.kind,
            row.priority,
            row.subject,
            row.body,
            row.thread_id,
            row.reply_to,
            row.outcome,
            row.settles_task_id,
            row.from_session_id,
            row.delivery,
            row.read_at,
            row.created_at
        ],
    )
}

impl Store {
    /// Stores a message that settles nothing. An unknown task is `StoreError::Constraint`.
    pub fn insert_team_message(&self, row: &TeamMessageRow) -> Result<(), StoreError> {
        insert(&self.lock(), row)?;
        Ok(())
    }

    /// Stores a `worker_done` that settles `task` for its run `session`, and latches `outcome` into the task when that run
    /// has not ended it yet, in one transaction, so two reports racing settle the run once. `row` must name `task` and
    /// `session` as its settled task and sending run. A reopened task has a new run, so its next report settles again.
    pub fn insert_worker_done(
        &self,
        row: &TeamMessageRow,
        task: &str,
        session: &str,
        outcome: TaskOutcome,
    ) -> Result<WorkerDoneStored, StoreError> {
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        let first = transaction
            .query_row(
                &format!("{SELECT_MESSAGE} WHERE settles_task_id = ?1 AND from_session_id = ?2"),
                params![task, session],
                message_from_row,
            )
            .optional()?;
        if let Some(first) = first {
            return Ok(WorkerDoneStored::Duplicate(Box::new(first)));
        }
        let latched = transaction.execute(
            "UPDATE tasks SET outcome = ?3, updated_at = ?4 WHERE id = ?1 AND session_id = ?2 AND outcome IS NULL",
            params![task, session, outcome, row.created_at],
        )?;
        let stored = if latched > 0 {
            WorkerDoneStored::Settled
        } else {
            let current = transaction
                .query_row("SELECT outcome FROM tasks WHERE id = ?1", [task], |found| {
                    found.get::<_, Option<TaskOutcome>>(0)
                })
                .optional()?
                .flatten();
            if current != Some(outcome) {
                return Ok(WorkerDoneStored::Conflict(current));
            }
            WorkerDoneStored::Matched
        };
        insert(&transaction, row)?;
        transaction.commit()?;
        Ok(stored)
    }

    /// One message by id.
    pub fn get_team_message(&self, id: &str) -> Result<Option<TeamMessageRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_MESSAGE} WHERE id = ?1"),
            [id],
            message_from_row,
        )
    }

    /// The messages sent to `task`, oldest first; only the unread ones when `unread_only`.
    pub fn team_inbox(
        &self,
        task: &str,
        unread_only: bool,
    ) -> Result<Vec<TeamMessageRow>, StoreError> {
        let filter = if unread_only {
            " AND read_at IS NULL"
        } else {
            ""
        };
        self.query_all(
            &format!("{SELECT_MESSAGE} WHERE to_task_id = ?1{filter} ORDER BY created_at, id"),
            [task],
            message_from_row,
        )
    }

    /// Marks the messages `ids` sent to `task` read; a message to another task is left alone. Returns how many changed.
    pub fn mark_team_messages_read(
        &self,
        task: &str,
        ids: &[String],
        now: i64,
    ) -> Result<usize, StoreError> {
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        let mut changed = 0;
        for id in ids {
            changed += transaction.execute(
                "UPDATE team_messages SET read_at = ?3 WHERE id = ?1 AND to_task_id = ?2 AND read_at IS NULL",
                params![id, task, now],
            )?;
        }
        transaction.commit()?;
        Ok(changed)
    }

    /// Whether `thread` holds a message between tasks `a` and `b`, in either direction.
    pub fn team_thread_between(&self, thread: &str, a: &str, b: &str) -> Result<bool, StoreError> {
        Ok(self
            .query_optional(
                "SELECT 1 FROM team_messages WHERE thread_id = ?1 AND ((from_task_id = ?2 AND to_task_id = ?3) OR (from_task_id = ?3 AND to_task_id = ?2)) LIMIT 1",
                params![thread, a, b],
                |row| row.get::<_, i64>(0),
            )?
            .is_some())
    }

    /// Records where a message went (`inbox` or `run`).
    pub fn set_team_message_delivery(&self, id: &str, delivery: &str) -> Result<(), StoreError> {
        self.lock().execute(
            "UPDATE team_messages SET delivery = ?2 WHERE id = ?1",
            params![id, delivery],
        )?;
        Ok(())
    }

    /// The newest message of `kind` that `from`'s run `session` sent to `to`.
    pub fn latest_team_message(
        &self,
        from: &str,
        session: &str,
        to: &str,
        kind: &str,
    ) -> Result<Option<TeamMessageRow>, StoreError> {
        self.query_optional(
            &format!(
                "{SELECT_MESSAGE} WHERE from_task_id = ?1 AND from_session_id = ?2 AND to_task_id = ?3 AND kind = ?4 ORDER BY created_at DESC, id DESC LIMIT 1"
            ),
            params![from, session, to, kind],
            message_from_row,
        )
    }
}
