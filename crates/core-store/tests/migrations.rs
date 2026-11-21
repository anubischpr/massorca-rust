use std::fs;
use std::path::{Path, PathBuf};

use massorca_core_store::{MIGRATIONS, Store, StoreError};
use rusqlite::Connection;

fn latest() -> i64 {
    i64::try_from(MIGRATIONS.len()).unwrap()
}

fn store_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("store.sqlite3")
}

fn user_version(path: &Path) -> i64 {
    let connection = Connection::open(path).unwrap();
    connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap()
}

fn table_exists(path: &Path, table: &str) -> bool {
    let connection = Connection::open(path).unwrap();
    connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )
        .unwrap()
}

/// File names of every backup kept next to the store file.
fn backups_of(path: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.contains(".bak-v"))
        .collect();
    names.sort();
    names
}

#[test]
fn empty_file_migrates_to_the_latest_schema_without_a_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(&dir);
    drop(Store::open(&path).unwrap());

    assert_eq!(user_version(&path), latest());
    for table in ["projects", "workspaces", "settings"] {
        assert!(table_exists(&path, table), "missing table {table}");
    }
    assert!(backups_of(&path).is_empty());
}

#[test]
fn newer_schema_is_refused_and_the_file_is_left_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(&dir);
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("CREATE TABLE future_table (x INTEGER); PRAGMA user_version = 99;")
            .unwrap();
    }
    let before = fs::read(&path).unwrap();

    let error = Store::open(&path).unwrap_err();
    assert!(
        matches!(error, StoreError::NewerSchema { found: 99, supported } if supported == latest()),
        "{error:?}"
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(backups_of(&path).is_empty());
}

#[test]
fn older_non_empty_database_is_backed_up_before_it_is_migrated() {
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(&dir);
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("CREATE TABLE legacy (value TEXT); INSERT INTO legacy VALUES ('kept');")
            .unwrap();
    }
    drop(Store::open(&path).unwrap());

    assert_eq!(backups_of(&path), ["store.sqlite3.bak-v0"]);
    let backup = path.with_file_name("store.sqlite3.bak-v0");
    assert_eq!(user_version(&backup), 0);
    assert!(table_exists(&backup, "legacy"));
    assert!(!table_exists(&backup, "projects"));
    {
        let connection = Connection::open(&backup).unwrap();
        let value: String = connection
            .query_row("SELECT value FROM legacy", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, "kept");
    }

    assert_eq!(user_version(&path), latest());
    assert!(table_exists(&path, "legacy"));
}

#[test]
fn file_store_uses_write_ahead_logging() {
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(&dir);
    let store = Store::open(&path).unwrap();

    let connection = Connection::open(&path).unwrap();
    let mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    drop(connection);
    drop(store);
}
