use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{OptionalExtension, Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// Where a task came from: the Conference room, or the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskOrigin {
    Room,
    Cli,
}

impl TaskOrigin {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Room => "room",
            Self::Cli => "cli",
        }
    }
}

impl ToSql for TaskOrigin {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

impl FromSql for TaskOrigin {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "room" => Ok(Self::Room),
            "cli" => Ok(Self::Cli),
            other => Err(FromSqlError::Other(
                format!("unknown task origin {other:?}").into(),
            )),
        }
    }
}

/// The latched final state of a task. `Success` also records a task marked done by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskOutcome {
    Success,
    Failure,
    Cancelled,
    Interrupted,
}

impl TaskOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }
}

impl ToSql for TaskOutcome {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

impl FromSql for TaskOutcome {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "success" => Ok(Self::Success),
            "failure" => Ok(Self::Failure),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            other => Err(FromSqlError::Other(
                format!("unknown task outcome {other:?}").into(),
            )),
        }
    }
}

/// One row of `tasks`. Its team never changes; its agent session is set only after the prompt was delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRow {
    pub id: String,
    pub team_id: String,
    pub project_id: Option<String>,
    pub bot_id: Option<String>,
    pub title: String,
    pub body: String,
    pub origin: TaskOrigin,
    pub session_id: Option<String>,
    pub outcome: Option<TaskOutcome>,
    pub error: Option<String>,
    /// The protocol code of `error`, such as `agent.not_installed`.
    pub error_code: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

const SELECT_TASKS: &str = "SELECT id, team_id, project_id, bot_id, title, body, origin, session_id, outcome, error, error_code, created_at, updated_at FROM tasks";

fn task_from_row(row: &Row<'_>) -> rusqlite::Result<TaskRow> {
    Ok(TaskRow {
        id: row.get("id")?,
        team_id: row.get("team_id")?,
        project_id: row.get("project_id")?,
        bot_id: row.get("bot_id")?,
        title: row.get("title")?,
        body: row.get("body")?,
        origin: row.get("origin")?,
        session_id: row.get("session_id")?,
        outcome: row.get("outcome")?,
        error: row.get("error")?,
        error_code: row.get("error_code")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

/// The details `update_task_details` changes; `None` keeps a field as it is.
#[derive(Debug, Default, Clone, Copy)]
pub struct TaskDetails<'a> {
    pub project: Option<Option<&'a str>>,
    pub bot: Option<Option<&'a str>>,
    pub title: Option<&'a str>,
    pub body: Option<&'a str>,
}

impl Store {
    /// Inserts a task. Its team, bot and project must exist; a missing one is `StoreError::Constraint`.
    pub fn insert_task(&self, row: &TaskRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO tasks (id, team_id, project_id, bot_id, title, body, origin, session_id, outcome, error, error_code, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                row.id,
                row.team_id,
                row.project_id,
                row.bot_id,
                row.title,
                row.body,
                row.origin,
                row.session_id,
                row.outcome,
                row.error,
                row.error_code,
                row.created_at,
                row.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn get_task(&self, id: &str) -> Result<Option<TaskRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_TASKS} WHERE id = ?1"),
            [id],
            task_from_row,
        )
    }

    /// The tasks of one team, or of every team when `team_id` is `None`, in creation order.
    pub fn list_tasks(&self, team_id: Option<&str>) -> Result<Vec<TaskRow>, StoreError> {
        match team_id {
            Some(team_id) => self.query_all(
                &format!("{SELECT_TASKS} WHERE team_id = ?1 ORDER BY created_at, id"),
                [team_id],
                task_from_row,
            ),
            None => self.query_all(
                &format!("{SELECT_TASKS} ORDER BY created_at, id"),
                [],
                task_from_row,
            ),
        }
    }

    /// The tasks linked to one bot, in creation order.
    pub fn list_tasks_of_bot(&self, bot_id: &str) -> Result<Vec<TaskRow>, StoreError> {
        self.query_all(
            &format!("{SELECT_TASKS} WHERE bot_id = ?1 ORDER BY created_at, id"),
            [bot_id],
            task_from_row,
        )
    }

    /// The tasks linked to one project, in creation order.
    pub fn list_tasks_of_project(&self, project_id: &str) -> Result<Vec<TaskRow>, StoreError> {
        self.query_all(
            &format!("{SELECT_TASKS} WHERE project_id = ?1 ORDER BY created_at, id"),
            [project_id],
            task_from_row,
        )
    }

    /// Writes every column of a task but its start attempt. Returns whether a row changed. Handlers use the targeted writes
    /// below, which never overwrite a concurrent link; this one is for fixtures.
    pub fn update_task(&self, row: &TaskRow) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE tasks SET project_id = ?2, bot_id = ?3, title = ?4, body = ?5, session_id = ?6, outcome = ?7, error = ?8, error_code = ?10, updated_at = ?9 WHERE id = ?1",
            params![
                row.id,
                row.project_id,
                row.bot_id,
                row.title,
                row.body,
                row.session_id,
                row.outcome,
                row.error,
                row.updated_at,
                row.error_code
            ],
        )?;
        Ok(changed > 0)
    }

    /// Changes only the given details: `Some` sets a field (`Some(None)` clears a project or bot), `None` keeps it. The
    /// session, outcome and error have their own writes, so an update never overwrites a concurrent link or a concurrent
    /// change to another detail. Returns whether a row changed.
    pub fn update_task_details(
        &self,
        id: &str,
        changes: &TaskDetails<'_>,
        updated_at: i64,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE tasks SET                 project_id = CASE WHEN ?2 THEN ?3 ELSE project_id END,                 bot_id = CASE WHEN ?4 THEN ?5 ELSE bot_id END,                 title = COALESCE(?6, title), body = COALESCE(?7, body), updated_at = ?8              WHERE id = ?1",
            params![
                id,
                changes.project.is_some(),
                changes.project.flatten(),
                changes.bot.is_some(),
                changes.bot.flatten(),
                changes.title,
                changes.body,
                updated_at
            ],
        )?;
        Ok(changed > 0)
    }

    /// Latches a task's outcome if its session is still `expected_session`. Returns false when the task is gone or its
    /// session changed since the caller read it.
    pub fn set_task_outcome(
        &self,
        id: &str,
        outcome: TaskOutcome,
        expected_session: Option<&str>,
        updated_at: i64,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE tasks SET outcome = ?2, updated_at = ?4 WHERE id = ?1 AND session_id IS ?3",
            params![id, outcome, expected_session, updated_at],
        )?;
        Ok(changed > 0)
    }

    /// Clears a task's outcome, error and session link if its session is still `expected_session`. Returns false when the
    /// task is gone or its session changed since the caller read it.
    pub fn reopen_task(
        &self,
        id: &str,
        expected_session: Option<&str>,
        updated_at: i64,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE tasks SET outcome = NULL, error = NULL, error_code = NULL, session_id = NULL, updated_at = ?3 WHERE id = ?1 AND session_id IS ?2",
            params![id, expected_session, updated_at],
        )?;
        Ok(changed > 0)
    }

    /// The task's start attempt: the one still pending, or `candidate` when none is. `None` for an unknown task.
    /// A retry of an unsettled attempt reuses it, so it names the same agent request.
    pub fn begin_task_start_attempt(
        &self,
        id: &str,
        candidate: &str,
    ) -> Result<Option<String>, StoreError> {
        let connection = self.lock();
        connection.execute(
            "UPDATE tasks SET start_attempt = ?2 WHERE id = ?1 AND start_attempt IS NULL",
            params![id, candidate],
        )?;
        Ok(connection
            .query_row(
                "SELECT start_attempt FROM tasks WHERE id = ?1",
                [id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten())
    }

    /// Clears a start attempt that ended with no live process. Returns false when the task is gone or holds another one.
    pub fn end_task_start_attempt(&self, id: &str, attempt: &str) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE tasks SET start_attempt = NULL WHERE id = ?1 AND start_attempt = ?2",
            params![id, attempt],
        )?;
        Ok(changed > 0)
    }

    /// Links a task that has no session yet to `session_id`, clears its error and settles its start attempt. Returns false
    /// when the task is gone or another start linked it first.
    pub fn link_task_session(
        &self,
        id: &str,
        session_id: &str,
        updated_at: i64,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE tasks SET session_id = ?2, error = NULL, error_code = NULL, start_attempt = NULL, updated_at = ?3 WHERE id = ?1 AND session_id IS NULL",
            params![id, session_id, updated_at],
        )?;
        Ok(changed > 0)
    }

    /// Records why a start failed, with its protocol code, on a task that has no session. Returns false when the task is
    /// gone or was linked.
    pub fn set_task_start_error(
        &self,
        id: &str,
        code: &str,
        error: &str,
        updated_at: i64,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE tasks SET error = ?2, error_code = ?3, updated_at = ?4 WHERE id = ?1 AND session_id IS NULL",
            params![id, error, code, updated_at],
        )?;
        Ok(changed > 0)
    }

    /// Deletes a task. Returns whether a row was removed.
    pub fn delete_task(&self, id: &str) -> Result<bool, StoreError> {
        let removed = self
            .lock()
            .execute("DELETE FROM tasks WHERE id = ?1", [id])?;
        Ok(removed > 0)
    }
}
