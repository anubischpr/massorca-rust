//! Steps kept one file each under `crates/core-store/migrations`, applied after the frozen list (decision 0119).
//! Each applied step's id is recorded in `store_migrations`, so steps may apply in any id order.

use std::collections::HashSet;
use std::path::Path;

use rusqlite::{Connection, TransactionBehavior, params};
use tracing::warn;

use crate::error::StoreError;
use crate::migrations::{
    back_up_before_first_step, first_foreign_key_violation, user_version, with_foreign_keys_off,
};

/// A step kept in its own file; `id` is the file name's `<block>-<yyyymmddhhmm>` part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileMigration {
    pub(crate) id: &'static str,
    pub(crate) sql: &'static str,
    pub(crate) foreign_keys_off: bool,
}

include!(concat!(env!("OUT_DIR"), "/file_migrations.rs"));

/// Created by the first file step that runs, not by a frozen step: the frozen list is pinned, and a database with no
/// file steps keeps the schema it had.
const CREATE_TRACKING_TABLE: &str = "CREATE TABLE IF NOT EXISTS store_migrations (
    id TEXT PRIMARY KEY,
    applied_at INTEGER NOT NULL
);";

/// Applies every file step not yet recorded, in id order, each in its own transaction. Runs under the caller's
/// migration lock; the backup taken before the first change is shared with the frozen steps through `backed_up`.
pub(crate) fn apply_pending(
    connection: &mut Connection,
    file: Option<&Path>,
    steps: &[FileMigration],
    backed_up: &mut bool,
    observe: &mut dyn FnMut(&'static str),
) -> Result<(), StoreError> {
    for step in pending(connection, steps)? {
        with_foreign_keys_off(connection, step.foreign_keys_off, |connection| {
            run_step(connection, file, step, backed_up)
        })?;
        observe("store schema step");
    }
    Ok(())
}

/// The file steps not yet recorded, in id order. An id recorded but unknown to this build is logged, not refused: a
/// step from another branch may have been applied to this database.
pub(crate) fn pending<'steps>(
    connection: &Connection,
    steps: &'steps [FileMigration],
) -> Result<Vec<&'steps FileMigration>, StoreError> {
    let applied = applied_ids(connection)?;
    for id in applied
        .iter()
        .filter(|id| steps.iter().all(|step| step.id != id.as_str()))
    {
        warn!(migration = %id, "database has a migration this build does not know; it stays applied");
    }
    let mut pending: Vec<&FileMigration> = steps
        .iter()
        .filter(|step| !applied.contains(step.id))
        .collect();
    pending.sort_by_key(|step| step.id);
    Ok(pending)
}

fn applied_ids(connection: &Connection) -> Result<HashSet<String>, StoreError> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'store_migrations')",
        [],
        |row| row.get(0),
    )?;
    if !exists {
        return Ok(HashSet::new());
    }
    let mut statement = connection.prepare("SELECT id FROM store_migrations")?;
    let ids = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<HashSet<String>>>()?;
    Ok(ids)
}

/// Runs the step if it is still unrecorded under the write lock: another opener may have applied it while this one
/// waited. The backup comes before the tracking table is created, so it sees the database as it was.
fn run_step(
    connection: &mut Connection,
    file: Option<&Path>,
    step: &FileMigration,
    backed_up: &mut bool,
) -> Result<(), StoreError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if applied_ids(&transaction)?.contains(step.id) {
        return Ok(());
    }
    let found = user_version(&transaction)?;
    back_up_before_first_step(&transaction, file, found, backed_up)?;
    transaction.execute_batch(CREATE_TRACKING_TABLE)?;
    transaction.execute_batch(step.sql)?;
    if step.foreign_keys_off
        && let Some(table) = first_foreign_key_violation(&transaction)?
    {
        return Err(StoreError::FileMigrationForeignKeyViolation {
            id: step.id.to_owned(),
            table,
        });
    }
    transaction.execute(
        "INSERT INTO store_migrations (id, applied_at) VALUES (?1, ?2)",
        params![step.id, crate::now_ms()],
    )?;
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests;
