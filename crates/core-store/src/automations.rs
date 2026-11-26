use rusqlite::{Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// One row of `automations` (decision 0123): a scheduled agent run, defined and listed. Runs are not stored yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationRow {
    pub id: String,
    /// The project the runs work in; `None` once that project is removed, so the automation never starts.
    pub project_id: Option<String>,
    pub name: String,
    pub prompt: String,
    /// The agent kind id, such as `claude`.
    pub agent_kind: String,
    /// `hourly`, `daily`, `weekdays`, `weekly` or `cron`.
    pub cadence: String,
    /// Wall-clock `HH:MM`, 24-hour.
    pub time: String,
    /// `monday` to `sunday`, for a weekly automation.
    pub weekday: Option<String>,
    /// The five-field expression, for a cron automation.
    pub cron: Option<String>,
    /// An IANA zone name; `None` is the host's zone.
    pub zone: Option<String>,
    /// Minutes a missed run may be late and still run; `None` is the default grace.
    pub grace_minutes: Option<i64>,
    pub enabled: bool,
    /// The client's key for a create it may retry; a repeat with the same key returns the automation it made.
    pub creation_key: Option<String>,
    /// The next occurrence to start, UTC epoch milliseconds; `None` while disabled.
    pub next_run_at: Option<i64>,
    /// When the last run started; `None` until runs are stored.
    pub last_run_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

const SELECT_AUTOMATIONS: &str = "SELECT id, project_id, name, prompt, agent_kind, cadence, time, weekday, cron, zone, grace_minutes, enabled, creation_key, next_run_at, last_run_at, created_at, updated_at FROM automations";

fn automation_from_row(row: &Row<'_>) -> rusqlite::Result<AutomationRow> {
    Ok(AutomationRow {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        name: row.get("name")?,
        prompt: row.get("prompt")?,
        agent_kind: row.get("agent_kind")?,
        cadence: row.get("cadence")?,
        time: row.get("time")?,
        weekday: row.get("weekday")?,
        cron: row.get("cron")?,
        zone: row.get("zone")?,
        grace_minutes: row.get("grace_minutes")?,
        enabled: row.get::<_, i64>("enabled")? != 0,
        creation_key: row.get("creation_key")?,
        next_run_at: row.get("next_run_at")?,
        last_run_at: row.get("last_run_at")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

impl Store {
    /// Inserts an automation. An unknown project, a cadence the table refuses, or a grace window out of range is
    /// `StoreError::Constraint`, and a repeated creation key is `StoreError::Conflict`.
    pub fn insert_automation(&self, row: &AutomationRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO automations (id, project_id, name, prompt, agent_kind, cadence, time, weekday, cron, zone, grace_minutes, enabled, creation_key, next_run_at, last_run_at, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                row.id,
                row.project_id,
                row.name,
                row.prompt,
                row.agent_kind,
                row.cadence,
                row.time,
                row.weekday,
                row.cron,
                row.zone,
                row.grace_minutes,
                i64::from(row.enabled),
                row.creation_key,
                row.next_run_at,
                row.last_run_at,
                row.created_at,
                row.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn get_automation(&self, id: &str) -> Result<Option<AutomationRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_AUTOMATIONS} WHERE id = ?1"),
            [id],
            automation_from_row,
        )
    }

    /// The automation that was created with `key`, if any.
    pub fn find_automation_by_creation_key(
        &self,
        key: &str,
    ) -> Result<Option<AutomationRow>, StoreError> {
        self.query_optional(
            &format!("{SELECT_AUTOMATIONS} WHERE creation_key = ?1"),
            [key],
            automation_from_row,
        )
    }

    /// Every automation, in creation order.
    pub fn list_automations(&self) -> Result<Vec<AutomationRow>, StoreError> {
        self.query_all(
            &format!("{SELECT_AUTOMATIONS} ORDER BY created_at, id"),
            [],
            automation_from_row,
        )
    }

    /// Writes the editable fields of the row with `row.id`. Returns `false`, and writes nothing, when no such row
    /// exists. The creation key and the creation time never change.
    pub fn update_automation(&self, row: &AutomationRow) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE automations SET project_id = ?2, name = ?3, prompt = ?4, agent_kind = ?5, cadence = ?6, time = ?7, weekday = ?8, cron = ?9, zone = ?10, grace_minutes = ?11, enabled = ?12, next_run_at = ?13, last_run_at = ?14, updated_at = ?15 WHERE id = ?1",
            params![
                row.id,
                row.project_id,
                row.name,
                row.prompt,
                row.agent_kind,
                row.cadence,
                row.time,
                row.weekday,
                row.cron,
                row.zone,
                row.grace_minutes,
                i64::from(row.enabled),
                row.next_run_at,
                row.last_run_at,
                row.updated_at
            ],
        )?;
        Ok(changed > 0)
    }

    /// Deletes an automation. Returns whether a row was removed. Runs are not stored yet, so there is no history to
    /// remove with it.
    pub fn delete_automation(&self, id: &str) -> Result<bool, StoreError> {
        let removed = self
            .lock()
            .execute("DELETE FROM automations WHERE id = ?1", [id])?;
        Ok(removed > 0)
    }
}
