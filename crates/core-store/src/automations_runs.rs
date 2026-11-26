use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// The states an automation run is stored in (decision 0125). A run in `DISPATCHING` or `DISPATCHED` is in progress;
/// every other state is final.
pub mod run_state {
    pub const DISPATCHING: &str = "dispatching";
    pub const DISPATCHED: &str = "dispatched";
    pub const COMPLETED: &str = "completed";
    pub const SKIPPED_MISSED: &str = "skipped_missed";
    pub const SKIPPED_UNAVAILABLE: &str = "skipped_unavailable";
    pub const DISPATCH_FAILED: &str = "dispatch_failed";
}

/// One row of `automation_runs`: a run of an automation, or several missed occurrences folded into one row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutomationRunRow {
    pub id: String,
    pub automation_id: String,
    /// One more than the automation's highest number, given by the store on insert. Retention keeps the newest runs, so
    /// a number is never given twice.
    pub number: i64,
    /// `schedule` or `manual`.
    pub trigger: String,
    /// The occurrence the run stands for, or when a manual run was asked for; UTC epoch milliseconds.
    pub scheduled_for: i64,
    /// How many occurrences the row stands for: more than one when missed occurrences are folded together.
    pub occurrences: i64,
    /// One of [`run_state`].
    pub state: String,
    /// `success`, `failure`, `cancelled` or `interrupted`, once the agent reported Done.
    pub outcome: Option<String>,
    pub session_id: Option<String>,
    pub workspace_id: Option<String>,
    /// Why the run was skipped or failed.
    pub reason: Option<String>,
    /// The protocol code of `reason`.
    pub reason_code: Option<String>,
    /// The agent's last result text, capped by the caller.
    pub result: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    /// When the agent session started.
    pub started_at: Option<i64>,
    /// When the run reached its final state.
    pub ended_at: Option<i64>,
    pub created_at: i64,
}

const SELECT_RUNS: &str = "SELECT id, automation_id, number, trigger, scheduled_for, occurrences, state, outcome, session_id, workspace_id, reason, reason_code, result, input_tokens, output_tokens, started_at, ended_at, created_at FROM automation_runs";

/// The states a run is in progress in, as an SQL list.
const IN_PROGRESS: &str = "('dispatching','dispatched')";

fn run_from_row(row: &Row<'_>) -> rusqlite::Result<AutomationRunRow> {
    Ok(AutomationRunRow {
        id: row.get("id")?,
        automation_id: row.get("automation_id")?,
        number: row.get("number")?,
        trigger: row.get("trigger")?,
        scheduled_for: row.get("scheduled_for")?,
        occurrences: row.get("occurrences")?,
        state: row.get("state")?,
        outcome: row.get("outcome")?,
        session_id: row.get("session_id")?,
        workspace_id: row.get("workspace_id")?,
        reason: row.get("reason")?,
        reason_code: row.get("reason_code")?,
        result: row.get("result")?,
        input_tokens: row.get("input_tokens")?,
        output_tokens: row.get("output_tokens")?,
        started_at: row.get("started_at")?,
        ended_at: row.get("ended_at")?,
        created_at: row.get("created_at")?,
    })
}

/// Inserts `row` with the next number of its automation, and writes that number back into `row`.
fn insert_run(connection: &Connection, row: &mut AutomationRunRow) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO automation_runs (id, automation_id, number, trigger, scheduled_for, occurrences, state, outcome, session_id, workspace_id, reason, reason_code, result, input_tokens, output_tokens, started_at, ended_at, created_at) \
         VALUES (?1, ?2, (SELECT COALESCE(MAX(number), 0) + 1 FROM automation_runs WHERE automation_id = ?2), ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            row.id,
            row.automation_id,
            row.trigger,
            row.scheduled_for,
            row.occurrences,
            row.state,
            row.outcome,
            row.session_id,
            row.workspace_id,
            row.reason,
            row.reason_code,
            row.result,
            row.input_tokens,
            row.output_tokens,
            row.started_at,
            row.ended_at,
            row.created_at
        ],
    )?;
    row.number = connection.query_row(
        "SELECT number FROM automation_runs WHERE id = ?1",
        [&row.id],
        |found| found.get(0),
    )?;
    Ok(())
}

fn set_last_run(
    connection: &Connection,
    automation_id: &str,
    last_run_at: Option<i64>,
) -> Result<(), StoreError> {
    if let Some(at) = last_run_at {
        connection.execute(
            "UPDATE automations SET last_run_at = ?2 WHERE id = ?1",
            params![automation_id, at],
        )?;
    }
    Ok(())
}

impl Store {
    /// Automations that are enabled and whose next run is at or before `now`, earliest first.
    pub fn due_automations(&self, now: i64) -> Result<Vec<crate::AutomationRow>, StoreError> {
        Ok(self
            .list_automations()?
            .into_iter()
            .filter(|row| row.enabled && row.next_run_at.is_some_and(|at| at <= now))
            .collect())
    }

    /// Claims one due occurrence: moves `next_run_at` from `expected_next` to `new_next`, sets `last_run_at` when given,
    /// and stores `runs` with their numbers, in one transaction. Returns `false`, and writes nothing, when `next_run_at`
    /// is no longer `expected_next`: another check claimed it.
    pub fn claim_automation_runs(
        &self,
        automation_id: &str,
        expected_next: i64,
        new_next: Option<i64>,
        last_run_at: Option<i64>,
        runs: &mut [AutomationRunRow],
    ) -> Result<bool, StoreError> {
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE automations SET next_run_at = ?1 WHERE id = ?2 AND next_run_at = ?3",
            params![new_next, automation_id, expected_next],
        )?;
        if changed == 0 {
            return Ok(false);
        }
        set_last_run(&transaction, automation_id, last_run_at)?;
        for run in runs.iter_mut() {
            insert_run(&transaction, run)?;
        }
        transaction.commit()?;
        Ok(true)
    }

    /// Stores a run outside the schedule, such as a manual one, and sets `last_run_at` when given. The schedule does
    /// not move.
    pub fn insert_automation_run(
        &self,
        row: &mut AutomationRunRow,
        last_run_at: Option<i64>,
    ) -> Result<(), StoreError> {
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        insert_run(&transaction, row)?;
        set_last_run(&transaction, &row.automation_id, last_run_at)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_automation_run(&self, id: &str) -> Result<Option<AutomationRunRow>, StoreError> {
        self.query_optional(&format!("{SELECT_RUNS} WHERE id = ?1"), [id], run_from_row)
    }

    /// A dispatching run's agent started: it is dispatched with its session and workspace. Returns whether it moved.
    pub fn mark_automation_run_dispatched(
        &self,
        id: &str,
        session_id: &str,
        workspace_id: &str,
        started_at: i64,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE automation_runs SET state = 'dispatched', session_id = ?2, workspace_id = ?3, started_at = ?4 WHERE id = ?1 AND state = 'dispatching'",
            params![id, session_id, workspace_id, started_at],
        )?;
        Ok(changed > 0)
    }

    /// A run in progress failed with `code` and `message`. A run already final keeps its state. Returns whether it
    /// moved.
    pub fn fail_automation_run(
        &self,
        id: &str,
        code: &str,
        message: &str,
        ended_at: i64,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            &format!(
                "UPDATE automation_runs SET state = 'dispatch_failed', reason_code = ?2, reason = ?3, ended_at = ?4 WHERE id = ?1 AND state IN {IN_PROGRESS}"
            ),
            params![id, code, message, ended_at],
        )?;
        Ok(changed > 0)
    }

    /// Ends the dispatched run of `session_id` in `state`, with its outcome, result and reason. Returns the ended run,
    /// or `None` when no dispatched run names the session, so a run ends once.
    pub fn end_automation_run_by_session(
        &self,
        session_id: &str,
        state: &str,
        outcome: Option<&str>,
        result: Option<&str>,
        reason: Option<(&str, &str)>,
        ended_at: i64,
    ) -> Result<Option<AutomationRunRow>, StoreError> {
        let (code, message) = reason.unzip();
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        let id: Option<String> = transaction
            .query_row(
                "SELECT id FROM automation_runs WHERE session_id = ?1 AND state = 'dispatched'",
                [session_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(id) = id else {
            return Ok(None);
        };
        transaction.execute(
            "UPDATE automation_runs SET state = ?2, outcome = ?3, result = ?4, reason_code = ?5, reason = ?6, ended_at = ?7 WHERE id = ?1",
            params![id, state, outcome, result, code, message, ended_at],
        )?;
        let ended =
            transaction.query_row(&format!("{SELECT_RUNS} WHERE id = ?1"), [&id], run_from_row)?;
        transaction.commit()?;
        Ok(Some(ended))
    }

    /// The runs of one automation, newest first: at most `limit`, numbered below `before` when it is given.
    pub fn list_automation_runs(
        &self,
        automation_id: &str,
        limit: usize,
        before: Option<i64>,
    ) -> Result<Vec<AutomationRunRow>, StoreError> {
        self.query_all(
            &format!(
                "{SELECT_RUNS} WHERE automation_id = ?1 AND number < ?2 ORDER BY number DESC LIMIT {limit}"
            ),
            params![automation_id, before.unwrap_or(i64::MAX)],
            run_from_row,
        )
    }

    /// Deletes the final runs of one automation beyond the newest `keep`; runs in progress are never deleted. Returns
    /// how many it deleted.
    pub fn prune_automation_runs(
        &self,
        automation_id: &str,
        keep: usize,
    ) -> Result<usize, StoreError> {
        let removed = self.lock().execute(
            &format!(
                "DELETE FROM automation_runs WHERE automation_id = ?1 AND state NOT IN {IN_PROGRESS} AND id NOT IN \
                 (SELECT id FROM automation_runs WHERE automation_id = ?1 AND state NOT IN {IN_PROGRESS} ORDER BY number DESC LIMIT {keep})"
            ),
            [automation_id],
        )?;
        Ok(removed)
    }

    /// Fails every run created before `created_before` that is still dispatching: the service stopped before its agent
    /// started or before the start was recorded. Dispatched runs are kept; their sessions settle them. Returns how many
    /// it settled.
    pub fn settle_dispatching_automation_runs(
        &self,
        created_before: i64,
        code: &str,
        message: &str,
        ended_at: i64,
    ) -> Result<usize, StoreError> {
        let settled = self.lock().execute(
            "UPDATE automation_runs SET state = 'dispatch_failed', reason_code = ?2, reason = ?3, ended_at = ?4 WHERE state = 'dispatching' AND created_at < ?1",
            params![created_before, code, message, ended_at],
        )?;
        Ok(settled)
    }
}
