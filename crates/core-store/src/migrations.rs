use std::path::Path;

use rusqlite::{Connection, TransactionBehavior};

use crate::backup;
use crate::error::StoreError;

/// Schema steps in order. The number applied is the database's `user_version`; shipped steps are never edited.
pub const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK(kind IN ('git','folder')),
    path TEXT NOT NULL,
    path_key TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL UNIQUE,
    workspaces_root TEXT NOT NULL,
    added_at INTEGER NOT NULL
);
CREATE TABLE workspaces (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    kind TEXT NOT NULL CHECK(kind IN ('worktree','folder')),
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    path_key TEXT NOT NULL UNIQUE,
    branch TEXT,
    base_ref TEXT,
    created_branch INTEGER NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('ready','removing')),
    request_id TEXT UNIQUE,
    created_at INTEGER NOT NULL,
    UNIQUE(project_id, name)
);
CREATE TABLE settings (
    owner TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL /* JSON */,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(owner, key)
);
"#];

/// Applies every step newer than the database's `user_version`. A newer database is refused before anything is written;
/// `file` is `Some` for on-disk databases, which are backed up before their first migration.
/// Each step runs under the write lock and re-reads `user_version` there, so concurrent openers migrate a file once.
pub(crate) fn migrate(
    connection: &mut Connection,
    file: Option<&Path>,
    steps: &[&str],
) -> Result<(), StoreError> {
    let supported = i64::try_from(steps.len()).unwrap_or(i64::MAX);
    let mut backed_up = false;
    loop {
        // Read without the lock first, so an up-to-date database is never written to.
        let found = user_version(connection)?;
        if found > supported {
            return Err(StoreError::NewerSchema { found, supported });
        }
        let index = usize::try_from(found).unwrap_or(0);
        let Some(step) = steps.get(index) else {
            return Ok(());
        };
        apply_step(connection, file, found, index, step, &mut backed_up)?;
    }
}

/// Runs one step if `user_version` still reads `found` under the write lock. Otherwise another process has moved
/// it on, and the caller reads it again.
fn apply_step(
    connection: &mut Connection,
    file: Option<&Path>,
    found: i64,
    index: usize,
    step: &str,
    backed_up: &mut bool,
) -> Result<(), StoreError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if user_version(&transaction)? != found {
        return Ok(());
    }
    // One backup per run, taken under the lock: a process that arrives after the schema moved on never copies it.
    if !*backed_up
        && let Some(file) = file
        && backup::holds_objects(&transaction)?
    {
        backup::create_before_migration(file, found)?;
    }
    *backed_up = true;
    transaction.execute_batch(step)?;
    transaction.execute_batch(&format!("PRAGMA user_version = {}", index + 1))?;
    transaction.commit()?;
    Ok(())
}

fn user_version(connection: &Connection) -> rusqlite::Result<i64> {
    connection.query_row("PRAGMA user_version", [], |row| row.get(0))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rusqlite::Connection;

    use super::migrate;

    const STEPS: &[&str] = &[
        "CREATE TABLE step_1 (x INTEGER);",
        "CREATE TABLE step_2 (x INTEGER);",
        "CREATE TABLE step_3 (x INTEGER);",
        "CREATE TABLE step_4 (x INTEGER);",
        "CREATE TABLE step_5 (x INTEGER);",
        "CREATE TABLE step_6 (x INTEGER);",
        "CREATE TABLE step_7 (x INTEGER);",
    ];

    #[test]
    fn only_the_five_newest_backups_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        let mut connection = Connection::open(&path).unwrap();
        migrate(&mut connection, Some(&path), &STEPS[..1]).unwrap();
        for version in 2..=STEPS.len() {
            // Each run migrates from version - 1, so each one backs up that old version.
            migrate(&mut connection, Some(&path), &STEPS[..version]).unwrap();
        }
        let mut backups: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.contains(".bak-v"))
            .collect();
        backups.sort();
        assert_eq!(
            backups,
            [
                "store.sqlite3.bak-v2",
                "store.sqlite3.bak-v3",
                "store.sqlite3.bak-v4",
                "store.sqlite3.bak-v5",
                "store.sqlite3.bak-v6",
            ]
        );
    }

    #[test]
    fn backup_holds_the_schema_from_before_the_migration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        let mut connection = Connection::open(&path).unwrap();
        migrate(&mut connection, Some(&path), &STEPS[..1]).unwrap();
        migrate(&mut connection, Some(&path), &STEPS[..2]).unwrap();
        drop(connection);

        let backup = Connection::open(dir.path().join("store.sqlite3.bak-v1")).unwrap();
        let version: i64 = backup
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
        let has_step_2: bool = backup
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE name = 'step_2')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!has_step_2);
    }
}
