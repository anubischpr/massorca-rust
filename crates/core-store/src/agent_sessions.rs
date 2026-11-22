//! Agent session rows (decision 0051): one row per session, with how its last run ended and its last activity.

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// Why a session's last run ended; the terminal vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunExitReason {
    Exited,
    Killed,
    ServiceStopped,
}

impl RunExitReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exited => "exited",
            Self::Killed => "killed",
            Self::ServiceStopped => "service_stopped",
        }
    }
}

impl ToSql for RunExitReason {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(self.as_str().into())
    }
}

impl FromSql for RunExitReason {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "exited" => Ok(Self::Exited),
            "killed" => Ok(Self::Killed),
            "service_stopped" => Ok(Self::ServiceStopped),
            other => Err(FromSqlError::Other(
                format!("unknown run exit reason {other:?}").into(),
            )),
        }
    }
}

/// How a session's last run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunExit {
    pub code: Option<i64>,
    pub signal: Option<String>,
    pub reason: RunExitReason,
    pub at: i64,
}

/// The last activity a session reported (0052). The values are the wire's (`working`, `done`, `hook`...), checked by
/// the table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredActivity {
    pub state: String,
    pub outcome: Option<String>,
    pub source: String,
    pub at: i64,
}

/// One row of `agent_sessions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSessionRow {
    pub id: String,
    pub kind: String,
    pub workspace_id: String,
    pub parent_id: Option<String>,
    pub title: Option<String>,
    pub read_only: bool,
    pub agent_ref: Option<String>,
    pub terminal_id: Option<String>,
    pub request_id: Option<String>,
    pub created_at: i64,
    /// `None` while the last run has not been seen to end.
    pub exit: Option<RunExit>,
    pub activity: Option<StoredActivity>,
    pub result: Option<String>,
    pub brought_back_at: Option<i64>,
}

const SELECT_SESSIONS: &str = "SELECT id, kind, workspace_id, parent_id, title, read_only, agent_ref, terminal_id, request_id, created_at, exit_code, exit_signal, exit_reason, exited_at, activity, activity_outcome, activity_source, activity_at, result, brought_back_at FROM agent_sessions";

fn session_from_row(row: &Row<'_>) -> rusqlite::Result<AgentSessionRow> {
    let exit = match row.get::<_, Option<RunExitReason>>("exit_reason")? {
        Some(reason) => Some(RunExit {
            code: row.get("exit_code")?,
            signal: row.get("exit_signal")?,
            reason,
            at: row.get::<_, Option<i64>>("exited_at")?.unwrap_or(0),
        }),
        None => None,
    };
    let activity = match row.get::<_, Option<String>>("activity")? {
        Some(state) => Some(StoredActivity {
            state,
            outcome: row.get("activity_outcome")?,
            source: row
                .get::<_, Option<String>>("activity_source")?
                .unwrap_or_default(),
            at: row.get::<_, Option<i64>>("activity_at")?.unwrap_or(0),
        }),
        None => None,
    };
    Ok(AgentSessionRow {
        id: row.get("id")?,
        kind: row.get("kind")?,
        workspace_id: row.get("workspace_id")?,
        parent_id: row.get("parent_id")?,
        title: row.get("title")?,
        read_only: row.get("read_only")?,
        agent_ref: row.get("agent_ref")?,
        terminal_id: row.get("terminal_id")?,
        request_id: row.get("request_id")?,
        created_at: row.get("created_at")?,
        exit,
        activity,
        result: row.get("result")?,
        brought_back_at: row.get("brought_back_at")?,
    })
}

impl Store {
    /// Inserts a session. Conflicts on id or request id are `StoreError::Conflict`; an unknown workspace or parent is
    /// `StoreError::Constraint`.
    pub fn insert_agent_session(&self, row: &AgentSessionRow) -> Result<(), StoreError> {
        let exit = row.exit.as_ref();
        let activity = row.activity.as_ref();
        self.lock().execute(
            "INSERT INTO agent_sessions (id, kind, workspace_id, parent_id, title, read_only, agent_ref, terminal_id, request_id, created_at, exit_code, exit_signal, exit_reason, exited_at, activity, activity_outcome, activity_source, activity_at, result, brought_back_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                row.id,
                row.kind,
                row.workspace_id,
                row.parent_id,
                row.title,
                row.read_only,
                row.agent_ref,
                row.terminal_id,
                row.request_id,
                row.created_at,
                exit.and_then(|exit| exit.code),
                exit.and_then(|exit| exit.signal.clone()),
                exit.map(|exit| exit.reason),
                exit.map(|exit| exit.at),
                activity.map(|activity| activity.state.clone()),
                activity.and_then(|activity| activity.outcome.clone()),
                activity.map(|activity| activity.source.clone()),
                activity.map(|activity| activity.at),
                row.result,
                row.brought_back_at
            ],
        )?;
        Ok(())
    }

    pub fn get_agent_session(&self, id: &str) -> Result<Option<AgentSessionRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_SESSIONS} WHERE id = ?1"),
            [id],
            session_from_row,
        )
    }

    pub fn find_agent_session_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Option<AgentSessionRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_SESSIONS} WHERE request_id = ?1"),
            [request_id],
            session_from_row,
        )
    }

    /// Sessions whose id starts with `prefix`, at most `limit` of them.
    pub fn find_agent_sessions_by_prefix(
        &self,
        prefix: &str,
        limit: u32,
    ) -> Result<Vec<AgentSessionRow>, StoreError> {
        // `substr` instead of LIKE, so `%` and `_` in the prefix match only themselves.
        self.query_all(
            &format!("{SELECT_SESSIONS} WHERE substr(id, 1, length(?1)) = ?1 ORDER BY id LIMIT ?2"),
            params![prefix, limit],
            session_from_row,
        )
    }

    /// Sessions newest first, optionally only those of one workspace.
    pub fn list_agent_sessions(
        &self,
        workspace_id: Option<&str>,
    ) -> Result<Vec<AgentSessionRow>, StoreError> {
        self.query_all(
            &format!(
                "{SELECT_SESSIONS} WHERE (?1 IS NULL OR workspace_id = ?1) ORDER BY created_at DESC, id DESC"
            ),
            [workspace_id],
            session_from_row,
        )
    }

    /// Records how the last run ended, unless an end is already recorded. Returns whether the row changed.
    pub fn set_agent_session_exit(&self, id: &str, exit: &RunExit) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE agent_sessions SET exit_code = ?2, exit_signal = ?3, exit_reason = ?4, exited_at = ?5 WHERE id = ?1 AND exit_reason IS NULL",
            params![id, exit.code, exit.signal, exit.reason, exit.at],
        )?;
        Ok(changed > 0)
    }

    /// Starts a new run of an exited session (0054): the row names the new run's terminal, and its exit and activity are
    /// cleared, so the activity starts absent until the agent reports. The result and the agent ref stay. One statement,
    /// so a reader never sees the exit cleared with the old terminal. Returns whether an exited row changed.
    pub fn restart_agent_session(&self, id: &str, terminal_id: &str) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE agent_sessions SET terminal_id = ?2, exit_code = NULL, exit_signal = NULL, exit_reason = NULL, exited_at = NULL, activity = NULL, activity_outcome = NULL, activity_source = NULL, activity_at = NULL WHERE id = ?1 AND exit_reason IS NOT NULL",
            params![id, terminal_id],
        )?;
        Ok(changed > 0)
    }

    /// Writes a session's activity and result (0052), replacing both, and sets its agent ref from `agent_ref_if_unset`
    /// only when the row has none. One statement, so a reader never sees half a change. Returns whether a row matched.
    pub fn set_agent_session_status(
        &self,
        id: &str,
        activity: Option<&StoredActivity>,
        result: Option<&str>,
        agent_ref_if_unset: Option<&str>,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE agent_sessions SET activity = ?2, activity_outcome = ?3, activity_source = ?4, activity_at = ?5, result = ?6, agent_ref = COALESCE(agent_ref, ?7) WHERE id = ?1",
            params![
                id,
                activity.map(|activity| activity.state.clone()),
                activity.and_then(|activity| activity.outcome.clone()),
                activity.map(|activity| activity.source.clone()),
                activity.map(|activity| activity.at),
                result,
                agent_ref_if_unset
            ],
        )?;
        Ok(changed > 0)
    }

    /// Marks every session whose run was not seen to end as ended by the service stopping (0051). Returns how many.
    pub fn mark_agent_runs_service_stopped(&self, at: i64) -> Result<u64, StoreError> {
        let changed = self.lock().execute(
            "UPDATE agent_sessions SET exit_code = NULL, exit_signal = NULL, exit_reason = 'service_stopped', exited_at = ?1 WHERE exit_reason IS NULL",
            [at],
        )?;
        Ok(u64::try_from(changed).unwrap_or(u64::MAX))
    }

    /// Returns whether a row was removed. Children keep their rows with no parent.
    pub fn delete_agent_session(&self, id: &str) -> Result<bool, StoreError> {
        let removed = self
            .lock()
            .execute("DELETE FROM agent_sessions WHERE id = ?1", [id])?;
        Ok(removed > 0)
    }
}
