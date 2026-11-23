use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{Connection, OptionalExtension, Row, params};

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
    /// The workspace the last start ran in.
    pub workspace_id: Option<String>,
    /// The branch of the task's own worktree.
    pub branch: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

const SELECT_TASKS: &str = "SELECT id, team_id, project_id, bot_id, title, body, origin, session_id, outcome, error, error_code, workspace_id, branch, created_at, updated_at FROM tasks";

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
        workspace_id: row.get("workspace_id")?,
        branch: row.get("branch")?,
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

/// Inserts a task on `connection`, so a caller's transaction can hold it with its own rows.
pub(crate) fn insert_task_row(connection: &Connection, row: &TaskRow) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO tasks (id, team_id, project_id, bot_id, title, body, origin, session_id, outcome, error, error_code, workspace_id, branch, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
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
            row.workspace_id,
            row.branch,
            row.created_at,
            row.updated_at
        ],
    )?;
    Ok(())
}

impl Store {
    /// Inserts a task. Its team, bot and project must exist; a missing one is `StoreError::Constraint`.
    pub fn insert_task(&self, row: &TaskRow) -> Result<(), StoreError> {
        insert_task_row(&self.lock(), row)
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

    /// Records where a start runs the task: its workspace, and the branch of its own worktree. Returns false when the
    /// task is gone.
    pub fn set_task_place(
        &self,
        id: &str,
        workspace_id: &str,
        branch: Option<&str>,
        updated_at: i64,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE tasks SET workspace_id = ?2, branch = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, workspace_id, branch, updated_at],
        )?;
        Ok(changed > 0)
    }

    /// The task's pending start attempt, if any.
    pub fn task_start_attempt(&self, id: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .lock()
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

    /// Links a task that has no session and no outcome yet to `session_id`, clears its error and settles its start attempt.
    /// Returns false when the task is gone, another start linked it first, or it was marked finished meanwhile.
    pub fn link_task_session(
        &self,
        id: &str,
        session_id: &str,
        updated_at: i64,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE tasks SET session_id = ?2, error = NULL, error_code = NULL, start_attempt = NULL, updated_at = ?3 WHERE id = ?1 AND session_id IS NULL AND outcome IS NULL",
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

    /// The task linked to `session_id`, if one is.
    pub fn find_task_by_session(&self, session_id: &str) -> Result<Option<TaskRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_TASKS} WHERE session_id = ?1"),
            [session_id],
            task_from_row,
        )
    }

    /// Latches a task's outcome, but only while the task is still linked to `session_id` and has none, so a reopened
    /// task or an earlier latch is never touched. Returns whether it latched.
    pub fn latch_task_outcome(
        &self,
        id: &str,
        session_id: &str,
        outcome: TaskOutcome,
        updated_at: i64,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE tasks SET outcome = ?3, updated_at = ?4 WHERE id = ?1 AND session_id = ?2 AND outcome IS NULL",
            params![id, session_id, outcome, updated_at],
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
