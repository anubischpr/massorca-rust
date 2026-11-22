use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{Row, params};

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
    pub created_at: i64,
    pub updated_at: i64,
}

const SELECT_TASKS: &str = "SELECT id, team_id, project_id, bot_id, title, body, origin, session_id, outcome, error, created_at, updated_at FROM tasks";

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
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

impl Store {
    /// Inserts a task. Its team, bot and project must exist; a missing one is `StoreError::Constraint`.
    pub fn insert_task(&self, row: &TaskRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO tasks (id, team_id, project_id, bot_id, title, body, origin, session_id, outcome, error, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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

    /// Changes a task's project, bot, title, body, session, outcome, error and update time. Returns whether a row changed.
    pub fn update_task(&self, row: &TaskRow) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE tasks SET project_id = ?2, bot_id = ?3, title = ?4, body = ?5, session_id = ?6, outcome = ?7, error = ?8, updated_at = ?9 WHERE id = ?1",
            params![
                row.id,
                row.project_id,
                row.bot_id,
                row.title,
                row.body,
                row.session_id,
                row.outcome,
                row.error,
                row.updated_at
            ],
        )?;
        Ok(changed > 0)
    }

    /// Links a task that has no session yet to `session_id` and clears its error. Returns false when the task is gone
    /// or another start linked it first; the whole-row `update_task` would overwrite a concurrent change.
    pub fn link_task_session(
        &self,
        id: &str,
        session_id: &str,
        updated_at: i64,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE tasks SET session_id = ?2, error = NULL, updated_at = ?3 WHERE id = ?1 AND session_id IS NULL",
            params![id, session_id, updated_at],
        )?;
        Ok(changed > 0)
    }

    /// Records why a start failed on a task that has no session. Returns false when the task is gone or was linked.
    pub fn set_task_start_error(
        &self,
        id: &str,
        error: &str,
        updated_at: i64,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE tasks SET error = ?2, updated_at = ?3 WHERE id = ?1 AND session_id IS NULL",
            params![id, error, updated_at],
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
