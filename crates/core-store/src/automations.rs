use rusqlite::{Row, params};

use crate::error::StoreError;
use crate::store::Store;

/// The destinations an automation's runs can work in (decision 0127), as stored.
pub mod destination {
    /// The project's own place: one worktree kept for the automation, or its folder's workspace (decision 0125).
    pub const PROJECT_DEFAULT: &str = "project_default";
    /// A new worktree for each run, in the project's repository.
    pub const PROJECT_WORKTREE_PER_RUN: &str = "project_worktree_per_run";
    /// One existing workspace, a worktree or a folder: every run goes there.
    pub const EXISTING_WORKSPACE: &str = "existing_workspace";
}

/// One row of `automations` (decision 0123): a scheduled agent run, defined and listed. Its runs are `automation_runs`.
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
    /// When the last run started; `None` until it first runs.
    pub last_run_at: Option<i64>,
    /// Where the runs work: one of [`destination`]. Rows stored before decision 0127 are `project_default`.
    pub destination: String,
    /// The existing workspace an `existing_workspace` automation runs in.
    pub destination_workspace_id: Option<String>,
    /// The workspace's path when the destination was set; a run finds it changed and is refused.
    pub destination_path: Option<String>,
    /// The base a per-run worktree starts from; `None` is the project's base.
    pub base_branch: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

const SELECT_AUTOMATIONS: &str = "SELECT id, project_id, name, prompt, agent_kind, cadence, time, weekday, cron, zone, grace_minutes, enabled, creation_key, next_run_at, last_run_at, destination, destination_workspace_id, destination_path, base_branch, created_at, updated_at FROM automations";

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
        destination: row.get("destination")?,
        destination_workspace_id: row.get("destination_workspace_id")?,
        destination_path: row.get("destination_path")?,
        base_branch: row.get("base_branch")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

impl Store {
    /// Inserts an automation. An unknown project, a cadence the table refuses, or a grace window out of range is
    /// `StoreError::Constraint`, and a repeated creation key is `StoreError::Conflict`.
    pub fn insert_automation(&self, row: &AutomationRow) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO automations (id, project_id, name, prompt, agent_kind, cadence, time, weekday, cron, zone, grace_minutes, enabled, creation_key, next_run_at, last_run_at, destination, destination_workspace_id, destination_path, base_branch, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
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
                row.destination,
                row.destination_workspace_id,
                row.destination_path,
                row.base_branch,
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
            "UPDATE automations SET project_id = ?2, name = ?3, prompt = ?4, agent_kind = ?5, cadence = ?6, time = ?7, weekday = ?8, cron = ?9, zone = ?10, grace_minutes = ?11, enabled = ?12, next_run_at = ?13, last_run_at = ?14, destination = ?15, destination_workspace_id = ?16, destination_path = ?17, base_branch = ?18, updated_at = ?19 WHERE id = ?1",
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
                row.destination,
                row.destination_workspace_id,
                row.destination_path,
                row.base_branch,
                row.updated_at
            ],
        )?;
        Ok(changed > 0)
    }

    /// Writes the definition fields of the row with `row.id`: never `next_run_at` or `last_run_at`, which a claim moves.
    /// Returns `false`, and writes nothing, when no such row exists.
    pub fn update_automation_definition(&self, row: &AutomationRow) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE automations SET project_id = ?2, name = ?3, prompt = ?4, agent_kind = ?5, cadence = ?6, time = ?7, weekday = ?8, cron = ?9, zone = ?10, grace_minutes = ?11, enabled = ?12, destination = ?13, destination_workspace_id = ?14, destination_path = ?15, base_branch = ?16, updated_at = ?17 WHERE id = ?1",
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
                row.destination,
                row.destination_workspace_id,
                row.destination_path,
                row.base_branch,
                row.updated_at
            ],
        )?;
        Ok(changed > 0)
    }

    /// Moves `next_run_at` to `new` only while it is still `expected`, so a claim made meanwhile is never undone.
    /// Returns whether it moved.
    pub fn set_automation_next_run_if(
        &self,
        id: &str,
        expected: Option<i64>,
        new: Option<i64>,
    ) -> Result<bool, StoreError> {
        let changed = self.lock().execute(
            "UPDATE automations SET next_run_at = ?3 WHERE id = ?1 AND next_run_at IS ?2",
            params![id, expected, new],
        )?;
        Ok(changed > 0)
    }

    /// Deletes an automation and its run history. Returns whether a row was removed.
    pub fn delete_automation(&self, id: &str) -> Result<bool, StoreError> {
        let removed = self
            .lock()
            .execute("DELETE FROM automations WHERE id = ?1", [id])?;
        Ok(removed > 0)
    }
}
