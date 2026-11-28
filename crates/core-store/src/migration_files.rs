//! Steps kept one file each under `crates/core-store/migrations`, applied after the frozen list (decision 0119).
//! Each applied step's id is recorded in `store_migrations`, so steps may apply in any id order.

use std::collections::HashSet;
use std::path::Path;

use rusqlite::{Connection, Transaction, TransactionBehavior, params};
use tracing::warn;

use crate::backup::BackupLabel;
use crate::error::StoreError;
use crate::migrations::{
    back_up_before_first_step, first_foreign_key_violation, with_foreign_keys_off,
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
/// `warn_unknown` is false when the caller's earlier `pending` call already logged unknown ids for this open.
pub(crate) fn apply_pending(
    connection: &mut Connection,
    file: Option<&Path>,
    steps: &[FileMigration],
    warn_unknown: bool,
    backed_up: &mut bool,
    observe: &mut dyn FnMut(&'static str),
) -> Result<(), StoreError> {
    for step in pending(connection, steps, warn_unknown)? {
        with_foreign_keys_off(connection, step.foreign_keys_off, |connection| {
            run_step(connection, file, step, backed_up)
        })?;
        observe("store schema step");
    }
    Ok(())
}

/// Applies and records every file step inside `transaction`, which is building a new database. Each step runs under a
/// savepoint: a failing one is rolled back alone and returned, so the caller commits the steps before it, as one
/// transaction per step would. Does nothing when a step needs foreign keys switched off, which cannot change inside a
/// transaction: `apply_pending` runs them all then.
pub(crate) fn apply_all_in(
    transaction: &Transaction<'_>,
    steps: &[FileMigration],
) -> Result<Option<StoreError>, StoreError> {
    if steps.is_empty() || steps.iter().any(|step| step.foreign_keys_off) {
        return Ok(None);
    }
    transaction.execute_batch(CREATE_TRACKING_TABLE)?;
    let mut ordered: Vec<&FileMigration> = steps.iter().collect();
    ordered.sort_by_key(|step| step.id);
    let applied_at = crate::now_ms();
    for step in ordered {
        transaction.execute_batch("SAVEPOINT file_step")?;
        let applied = transaction.execute_batch(step.sql).and_then(|()| {
            transaction.execute(
                "INSERT INTO store_migrations (id, applied_at) VALUES (?1, ?2)",
                params![step.id, applied_at],
            )
        });
        match applied {
            Ok(_) => transaction.execute_batch("RELEASE file_step")?,
            Err(error) => {
                transaction.execute_batch("ROLLBACK TO file_step; RELEASE file_step")?;
                return Ok(Some(error.into()));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
thread_local! {
    /// Unknown-id warnings logged on this thread; tests read it because tracing's callsite cache is process-wide.
    pub(crate) static UNKNOWN_ID_WARNINGS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The file steps not yet recorded, in id order. With `warn_unknown`, each recorded id unknown to this build is
/// logged, not refused: a newer build or another branch may have applied it. Shares the read that finds pending
/// steps, so the warning adds no query; an extra read can overlap another opener's commit.
pub(crate) fn pending<'steps>(
    connection: &Connection,
    steps: &'steps [FileMigration],
    warn_unknown: bool,
) -> Result<Vec<&'steps FileMigration>, StoreError> {
    let applied = applied_ids(connection)?;
    if warn_unknown {
        for id in applied
            .iter()
            .filter(|id| steps.iter().all(|step| step.id != id.as_str()))
        {
            warn!(migration = %id, "database has a migration this build does not know; it stays applied");
            #[cfg(test)]
            UNKNOWN_ID_WARNINGS.with(|count| count.set(count.get() + 1));
        }
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
    back_up_before_first_step(&transaction, file, BackupLabel::Step(step.id), backed_up)?;
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
