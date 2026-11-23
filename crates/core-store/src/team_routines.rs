use rusqlite::{Connection, Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// One row of `team_routines`: a schedule that starts a task for a bot on each occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamRoutineRow {
    pub id: String,
    pub team_id: String,
    /// The bot the runs' tasks are for; `None` once that bot is removed.
    pub bot_id: Option<String>,
    pub name: String,
    pub prompt: String,
    /// `daily`, `weekly`, `monthly` or `once`.
    pub cadence: String,
    /// Wall-clock `HH:MM`.
    pub time: String,
    /// `monday` to `sunday`, for a weekly routine.
    pub weekday: Option<String>,
    /// 1 to 28, for a monthly routine.
    pub month_day: Option<i64>,
    /// `YYYY-MM-DD`, for a once routine.
    pub run_date: Option<String>,
    /// An IANA zone name; `None` is the host's zone.
    pub zone: Option<String>,
    pub paused: bool,
    /// Whether a scheduled run receives vault secrets: the host-admin status of the caller that last added or updated it.
    pub starts_as_admin: bool,
    /// The next occurrence to claim, UTC epoch milliseconds; `None` while paused or once finished.
    pub next_run_at: Option<i64>,
    pub created_at: i64,
}

/// One row of `team_routine_runs`: an occurrence a routine claimed, or a manual run, and what it produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamRoutineRunRow {
    pub id: String,
    pub routine_id: String,
    /// The occurrence the run stands for, UTC epoch milliseconds.
    pub scheduled_for: i64,
    /// The task the run created; `None` when no task was made.
    pub task_id: Option<String>,
    /// Why the run or its task did not start.
    pub error: Option<String>,
    pub created_at: i64,
}

const SELECT_ROUTINES: &str = "SELECT id, team_id, bot_id, name, prompt, cadence, time, weekday, month_day, run_date, zone, paused, starts_as_admin, next_run_at, created_at FROM team_routines";
const SELECT_RUNS: &str =
    "SELECT id, routine_id, scheduled_for, task_id, error, created_at FROM team_routine_runs";

fn routine_from_row(row: &Row<'_>) -> rusqlite::Result<TeamRoutineRow> {
    Ok(TeamRoutineRow {
        id: row.get("id")?,
        team_id: row.get("team_id")?,
        bot_id: row.get("bot_id")?,
        name: row.get("name")?,
        prompt: row.get("prompt")?,
        cadence: row.get("cadence")?,
        time: row.get("time")?,
        weekday: row.get("weekday")?,
        month_day: row.get("month_day")?,
        run_date: row.get("run_date")?,
        zone: row.get("zone")?,
        paused: row.get::<_, i64>("paused")? != 0,
        starts_as_admin: row.get::<_, i64>("starts_as_admin")? != 0,
        next_run_at: row.get("next_run_at")?,
        created_at: row.get("created_at")?,
    })
}

fn run_from_row(row: &Row<'_>) -> rusqlite::Result<TeamRoutineRunRow> {
    Ok(TeamRoutineRunRow {
        id: row.get("id")?,
        routine_id: row.get("routine_id")?,
        scheduled_for: row.get("scheduled_for")?,
        task_id: row.get("task_id")?,
        error: row.get("error")?,
        created_at: row.get("created_at")?,
    })
}

fn insert_run(connection: &Connection, row: &TeamRoutineRunRow) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO team_routine_runs (id, routine_id, scheduled_for, task_id, error, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            row.id,
            row.routine_id,
            row.scheduled_for,
            row.task_id,
            row.error,
            row.created_at
        ],
    )?;
    Ok(())
}

impl Store {
    /// Inserts a routine. An unknown team is `StoreError::Constraint`, as is a cadence or month day the table refuses.
    pub fn insert_routine(&self, row: &TeamRoutineRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO team_routines (id, team_id, bot_id, name, prompt, cadence, time, weekday, month_day, run_date, zone, paused, starts_as_admin, next_run_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                row.id,
                row.team_id,
                row.bot_id,
                row.name,
                row.prompt,
                row.cadence,
                row.time,
                row.weekday,
                row.month_day,
                row.run_date,
                row.zone,
                row.paused,
                row.starts_as_admin,
                row.next_run_at,
                row.created_at
            ],
        )?;
        Ok(())
    }

    pub fn get_routine(&self, id: &str) -> Result<Option<TeamRoutineRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_ROUTINES} WHERE id = ?1"),
            [id],
            routine_from_row,
        )
    }

    /// The routines of one team, in creation order.
    pub fn list_routines(&self, team_id: &str) -> Result<Vec<TeamRoutineRow>, StoreError> {
        self.query_all(
            &format!("{SELECT_ROUTINES} WHERE team_id = ?1 ORDER BY created_at, id"),
            [team_id],
            routine_from_row,
        )
    }

    /// Routines whose next run is at or before `now`, earliest first. Paused and finished routines have no next run.
    pub fn due_routines(&self, now: i64) -> Result<Vec<TeamRoutineRow>, StoreError> {
        self.query_all(
            &format!(
                "{SELECT_ROUTINES} WHERE next_run_at IS NOT NULL AND next_run_at <= ?1 ORDER BY next_run_at, id"
            ),
            [now],
            routine_from_row,
        )
    }

    /// Writes every field a routine edit or a pause or resume changes. Returns whether a row changed.
    pub fn update_routine(&self, row: &TeamRoutineRow) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE team_routines SET bot_id = ?2, name = ?3, prompt = ?4, cadence = ?5, time = ?6, weekday = ?7, month_day = ?8, run_date = ?9, zone = ?10, paused = ?11, starts_as_admin = ?12, next_run_at = ?13 WHERE id = ?1",
            params![
                row.id,
                row.bot_id,
                row.name,
                row.prompt,
                row.cadence,
                row.time,
                row.weekday,
                row.month_day,
                row.run_date,
                row.zone,
                row.paused,
                row.starts_as_admin,
                row.next_run_at
            ],
        )?;
        Ok(changed > 0)
    }

    /// Deletes a routine and its runs. Returns whether a row was removed. The runs' tasks stay.
    pub fn delete_routine(&self, id: &str) -> Result<bool, StoreError> {
        let removed = self
            .lock()
            .execute("DELETE FROM team_routines WHERE id = ?1", [id])?;
        Ok(removed > 0)
    }

    /// Claims one occurrence: moves `next_run_at` from `expected_next` to `new_next` and records `run`, in one
    /// transaction. Returns `false`, and writes nothing, when `next_run_at` is no longer `expected_next`.
    pub fn claim_routine_run(
        &self,
        expected_next: i64,
        new_next: Option<i64>,
        run: &TeamRoutineRunRow,
    ) -> Result<bool, StoreError> {
        let mut connection = self.lock();
        let transaction = connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE team_routines SET next_run_at = ?1 WHERE id = ?2 AND next_run_at = ?3",
            params![new_next, run.routine_id, expected_next],
        )?;
        if changed == 0 {
            return Ok(false);
        }
        insert_run(&transaction, run)?;
        transaction.commit()?;
        Ok(true)
    }

    /// Records a run outside the schedule, such as a run made now. The schedule does not move.
    pub fn insert_routine_run(&self, row: &TeamRoutineRunRow) -> Result<(), StoreError> {
        insert_run(&self.lock(), row)
    }

    pub fn get_routine_run(&self, id: &str) -> Result<Option<TeamRoutineRunRow>, StoreError> {
        self.query_optional(&format!("{SELECT_RUNS} WHERE id = ?1"), [id], run_from_row)
    }

    /// Records the task a run made and why it did not start, if it did not. Returns whether a row changed.
    pub fn set_routine_run_result(
        &self,
        run_id: &str,
        task_id: Option<&str>,
        error: Option<&str>,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE team_routine_runs SET task_id = ?2, error = ?3 WHERE id = ?1",
            params![run_id, task_id, error],
        )?;
        Ok(changed > 0)
    }

    /// The runs of one routine, newest first; `limit` keeps the newest that many.
    pub fn list_routine_runs(
        &self,
        routine_id: &str,
        limit: usize,
    ) -> Result<Vec<TeamRoutineRunRow>, StoreError> {
        self.query_all(
            &format!(
                "{SELECT_RUNS} WHERE routine_id = ?1 ORDER BY scheduled_for DESC, created_at DESC, id DESC LIMIT {limit}"
            ),
            [routine_id],
            run_from_row,
        )
    }
}
