use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Barrier;
use std::thread;

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
    for table in ["projects", "workspaces", "settings", "agent_sessions"] {
        assert!(table_exists(&path, table), "missing table {table}");
    }
    assert!(backups_of(&path).is_empty());
}

#[test]
fn reopening_a_migrated_store_keeps_its_data_and_takes_no_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(&dir);
    Store::open(&path)
        .unwrap()
        .set_setting("app", "theme", "\"dark\"")
        .unwrap();

    let reopened = Store::open(&path).unwrap();
    assert_eq!(
        reopened.get_setting("app", "theme").unwrap(),
        Some("\"dark\"".to_owned())
    );
    drop(reopened);

    assert_eq!(user_version(&path), latest());
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
fn concurrent_opens_of_an_old_database_migrate_it_once() {
    for round in 0..200 {
        let dir = tempfile::tempdir().unwrap();
        let path = store_path(&dir);
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE legacy (value TEXT); INSERT INTO legacy VALUES ('kept');",
                )
                .unwrap();
        }

        let start = Barrier::new(2);
        let (first, second) = thread::scope(|scope| {
            let open = || {
                start.wait();
                Store::open(&path)
            };
            let first = scope.spawn(open);
            let second = scope.spawn(open);
            (first.join().unwrap(), second.join().unwrap())
        });
        first.unwrap_or_else(|error| panic!("round {round}: first open failed: {error}"));
        second.unwrap_or_else(|error| panic!("round {round}: second open failed: {error}"));

        assert_eq!(user_version(&path), latest(), "round {round}");
        assert_eq!(backups_of(&path), ["store.sqlite3.bak-v0"], "round {round}");
        let backup = path.with_file_name("store.sqlite3.bak-v0");
        assert_eq!(user_version(&backup), 0, "round {round}");
        assert!(!table_exists(&backup, "projects"), "round {round}");
        assert!(table_exists(&path, "legacy"), "round {round}");
    }
}

#[test]
fn in_memory_store_is_migrated_to_the_latest_schema() {
    let store = Store::open_in_memory().unwrap();
    store.set_setting("app", "theme", "\"dark\"").unwrap();
    assert_eq!(
        store.get_setting("app", "theme").unwrap(),
        Some("\"dark\"".to_owned())
    );
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

#[test]
fn a_version_1_store_gains_agent_sessions_and_keeps_its_workspaces() {
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(&dir);
    {
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(MIGRATIONS[0].sql).unwrap();
        connection
            .execute_batch(
                "INSERT INTO projects VALUES ('p1', 'git', '/repos/app', '/repos/app', 'app', '/ws', 1);
                 INSERT INTO workspaces VALUES ('w1', 'p1', 'worktree', 'main', '/ws/main', '/ws/main', 'main', 'main', 1, 'ready', NULL, 1);
                 PRAGMA user_version = 1;",
            )
            .unwrap();
    }

    let store = Store::open(&path).unwrap();
    assert!(store.get_workspace("w1").unwrap().is_some());
    assert!(store.list_agent_sessions(Some("w1")).unwrap().is_empty());
    drop(store);
    assert_eq!(user_version(&path), latest());
    assert!(table_exists(&path, "agent_sessions"));
    assert_eq!(
        backups_of(&path).len(),
        1,
        "migrating a used store backs it up"
    );
}

#[test]
fn every_step_is_plain_sql_and_a_fresh_file_has_the_account_and_vault_columns() {
    // A step's SQL once swallowed the Rust source of the next step, so the file would not open. Every step is SQL only.
    for (index, step) in MIGRATIONS.iter().enumerate() {
        assert!(
            !step.sql.contains("Migration {") && !step.sql.contains("\n    // "),
            "step {} holds Rust source instead of SQL only",
            index + 1
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(&dir);
    drop(Store::open(&path).unwrap());
    let connection = Connection::open(&path).unwrap();
    let has_column = |table: &str, column: &str| -> bool {
        connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2",
                [table, column],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
            == 1
    };
    assert!(
        has_column("agent_accounts", "origin"),
        "the origin column exists"
    );
    assert!(
        has_column("secret_requests", "requester"),
        "the vault requester column exists"
    );
    assert!(
        has_column("secret_key_check", "sealed"),
        "the vault key check table exists"
    );
    let found: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(found, latest());
}

#[test]
fn the_kept_account_step_keeps_the_history_recorded_before_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(&dir);
    // The step that records kept sign-ins, found by its SQL so later steps do not move it.
    let before = MIGRATIONS
        .iter()
        .position(|step| step.sql.contains("agent_accounts_record_kept_sign_in"))
        .expect("the kept-account step");
    {
        let connection = Connection::open(&path).unwrap();
        for step in &MIGRATIONS[..before] {
            connection.execute_batch(step.sql).unwrap();
        }
        connection
            .execute_batch(&format!(
                "INSERT INTO agent_account_history (kind) VALUES ('fake');
                 PRAGMA user_version = {before};"
            ))
            .unwrap();
    }

    let store = Store::open(&path).unwrap();
    assert!(
        store.kind_has_had_account("fake").unwrap(),
        "a record from before the step cannot tell kept from discarded, so it stays"
    );
    drop(store);
    assert_eq!(user_version(&path), latest());
}

#[test]
fn a_team_made_before_access_ran_bypass_modes_becomes_default_access() {
    let dir = tempfile::tempdir().unwrap();
    let path = store_path(&dir);
    let before = MIGRATIONS.len() - 1;
    {
        let connection = Connection::open(&path).unwrap();
        for step in &MIGRATIONS[..before] {
            connection.execute_batch(step.sql).unwrap();
        }
        connection
            .execute_batch(&format!(
                "PRAGMA user_version = {before};
                 INSERT INTO teams (id, name, purpose, character, access, workspaces, created_at)
                 VALUES ('t1', 'alpha', '', 'triangle', 'full', 0, 1);"
            ))
            .unwrap();
    }

    let store = Store::open(&path).unwrap();

    assert_eq!(user_version(&path), latest());
    assert_eq!(store.get_team("t1").unwrap().unwrap().access, "default");
}
