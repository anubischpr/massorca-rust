use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Barrier;
use std::thread;

use rusqlite::Connection;

use super::{FileMigration, UNKNOWN_ID_WARNINGS};
use crate::error::StoreError;
use crate::migrations::{Migration, migrate, migrate_observed};

const fn frozen(sql: &'static str) -> Migration {
    Migration {
        sql,
        foreign_keys_off: false,
    }
}

const fn file(id: &'static str, sql: &'static str) -> FileMigration {
    FileMigration {
        id,
        sql,
        foreign_keys_off: false,
    }
}

const FROZEN: &[Migration] = &[
    frozen("CREATE TABLE frozen_1 (x INTEGER);"),
    frozen("CREATE TABLE frozen_2 (x INTEGER);"),
];
const A: FileMigration = file("0110-202611261430", "CREATE TABLE step_a (x INTEGER);");
const B: FileMigration = file("0111-202611261430", "CREATE TABLE step_b (x INTEGER);");
const PARENT_AND_CHILD: Migration = frozen(
    "CREATE TABLE parent (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
     CREATE TABLE child (id INTEGER PRIMARY KEY, parent_id INTEGER NOT NULL REFERENCES parent (id));
     INSERT INTO parent VALUES (1, 'kept');
     INSERT INTO child VALUES (10, 1);",
);
const REBUILD_PARENT: FileMigration = FileMigration {
    id: "0112-202611261430",
    sql: "CREATE TABLE parent_new (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
          INSERT INTO parent_new SELECT id, name FROM parent;
          DROP TABLE parent;
          ALTER TABLE parent_new RENAME TO parent;",
    foreign_keys_off: true,
};
const DELETE_PARENT: FileMigration = FileMigration {
    id: "0113-202611261430",
    sql: "DELETE FROM parent;",
    foreign_keys_off: true,
};

/// Opens `path` with foreign keys enforced, as `Store` does, and runs both kinds of steps.
fn run(path: &Path, steps: &[Migration], files: &[FileMigration]) -> Result<(), StoreError> {
    let mut connection = Connection::open(path).unwrap();
    connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    migrate_observed(&mut connection, Some(path), steps, files, &mut |_| {})
}

fn database(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("store.db")
}

fn version(path: &Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap()
}

fn table_exists(path: &Path, table: &str) -> bool {
    Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [table],
            |row| row.get(0),
        )
        .unwrap()
}

/// Ids recorded in `store_migrations`, in the order they were applied.
fn applied(path: &Path) -> Vec<String> {
    let connection = Connection::open(path).unwrap();
    if !table_exists(path, "store_migrations") {
        return Vec::new();
    }
    let mut statement = connection
        .prepare("SELECT id FROM store_migrations ORDER BY rowid")
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// File names of every backup kept next to the database.
fn backups(path: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.contains(".bak-"))
        .collect();
    names.sort();
    names
}

#[test]
fn a_fresh_database_runs_the_frozen_steps_then_the_file_steps_in_id_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = database(&dir);

    run(&path, FROZEN, &[B, A]).unwrap();

    assert_eq!(version(&path), 2);
    for table in ["frozen_1", "frozen_2", "step_a", "step_b"] {
        assert!(table_exists(&path, table), "missing {table}");
    }
    assert_eq!(applied(&path), [A.id, B.id]);
}

#[test]
fn a_database_at_the_frozen_version_gets_the_file_steps() {
    let dir = tempfile::tempdir().unwrap();
    let path = database(&dir);
    migrate(&mut Connection::open(&path).unwrap(), Some(&path), FROZEN).unwrap();
    assert!(!table_exists(&path, "store_migrations"));

    run(&path, FROZEN, &[A]).unwrap();

    assert_eq!(version(&path), 2);
    assert!(table_exists(&path, "step_a"));
    assert_eq!(applied(&path), [A.id]);
}

#[test]
fn a_lower_id_added_after_a_higher_one_still_applies() {
    let dir = tempfile::tempdir().unwrap();
    let path = database(&dir);
    run(&path, FROZEN, &[B]).unwrap();

    run(&path, FROZEN, &[A, B]).unwrap();

    assert!(table_exists(&path, "step_a"));
    assert_eq!(applied(&path), [B.id, A.id]);
}

#[test]
fn a_failing_file_step_rolls_back_only_itself() {
    let dir = tempfile::tempdir().unwrap();
    let path = database(&dir);
    let broken = file(
        "0114-202611261430",
        "CREATE TABLE step_broken (x INTEGER); SELECT * FROM no_such_table;",
    );

    let error = run(&path, FROZEN, &[A, broken]).unwrap_err();

    assert!(matches!(error, StoreError::Sqlite(_)), "{error:?}");
    assert_eq!(applied(&path), [A.id]);
    assert!(table_exists(&path, "step_a"));
    assert!(!table_exists(&path, "step_broken"));
    assert_eq!(version(&path), 2);
}

#[test]
fn a_foreign_keys_off_file_step_can_rebuild_a_table_that_rows_reference() {
    let dir = tempfile::tempdir().unwrap();
    let path = database(&dir);
    run(&path, &[PARENT_AND_CHILD], &[]).unwrap();

    let mut connection = Connection::open(&path).unwrap();
    connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    migrate_observed(
        &mut connection,
        Some(&path),
        &[PARENT_AND_CHILD],
        &[REBUILD_PARENT],
        &mut |_| {},
    )
    .unwrap();

    assert_eq!(applied(&path), [REBUILD_PARENT.id]);
    let enforced: bool = connection
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .unwrap();
    assert!(enforced, "enforcement is switched back on after the step");
    let rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM child", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 1);
}

#[test]
fn a_foreign_keys_off_file_step_that_leaves_a_violation_is_rolled_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = database(&dir);
    run(&path, &[PARENT_AND_CHILD], &[]).unwrap();

    let error = run(&path, &[PARENT_AND_CHILD], &[DELETE_PARENT]).unwrap_err();

    assert!(
        matches!(
            error,
            StoreError::FileMigrationForeignKeyViolation { ref id, ref table }
                if id == DELETE_PARENT.id && table == "child"
        ),
        "{error:?}"
    );
    assert!(applied(&path).is_empty());
    let connection = Connection::open(&path).unwrap();
    let parents: i64 = connection
        .query_row("SELECT COUNT(*) FROM parent", [], |row| row.get(0))
        .unwrap();
    assert_eq!(parents, 1);
}

#[test]
fn a_legacy_database_is_backed_up_once_before_its_first_step() {
    let dir = tempfile::tempdir().unwrap();
    let path = database(&dir);
    Connection::open(&path)
        .unwrap()
        .execute_batch("CREATE TABLE legacy (value TEXT);")
        .unwrap();

    run(&path, FROZEN, &[A, B]).unwrap();

    assert_eq!(backups(&path), ["store.db.bak-v0"]);
    assert_eq!(applied(&path), [A.id, B.id]);
}

#[test]
fn a_file_only_upgrade_is_backed_up_once_for_all_its_steps() {
    let dir = tempfile::tempdir().unwrap();
    let path = database(&dir);
    run(&path, FROZEN, &[]).unwrap();
    assert!(
        backups(&path).is_empty(),
        "a new database has nothing to back up"
    );

    run(&path, FROZEN, &[A, B]).unwrap();

    assert_eq!(backups(&path), ["store.db.bak-0110-202611261430"]);
    assert_eq!(applied(&path), [A.id, B.id]);
}

#[test]
fn successive_file_only_upgrades_keep_each_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = database(&dir);
    run(&path, FROZEN, &[]).unwrap();
    run(&path, FROZEN, &[A]).unwrap();
    let first = fs::read(dir.path().join("store.db.bak-0110-202611261430")).unwrap();

    run(&path, FROZEN, &[A, B]).unwrap();

    assert_eq!(
        backups(&path),
        [
            "store.db.bak-0110-202611261430",
            "store.db.bak-0111-202611261430"
        ]
    );
    assert_eq!(
        fs::read(dir.path().join("store.db.bak-0110-202611261430")).unwrap(),
        first,
        "the backup taken before the first upgrade is not replaced"
    );
    assert_eq!(applied(&path), [A.id, B.id]);
}

#[test]
fn a_retry_after_the_first_step_fails_does_not_replace_its_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = database(&dir);
    run(&path, FROZEN, &[]).unwrap();
    let broken = file(
        "0114-202611261430",
        "CREATE TABLE step_broken (x INTEGER); SELECT * FROM no_such_table;",
    );
    run(&path, FROZEN, &[broken]).unwrap_err();
    let first = fs::read(dir.path().join("store.db.bak-0114-202611261430")).unwrap();
    Connection::open(&path)
        .unwrap()
        .execute_batch("CREATE TABLE changed_since_first_try (x INTEGER);")
        .unwrap();

    run(&path, FROZEN, &[broken]).unwrap_err();

    assert_eq!(
        fs::read(dir.path().join("store.db.bak-0114-202611261430")).unwrap(),
        first,
        "the first backup still holds the database as it was before the first try"
    );
    assert_eq!(
        backups(&path),
        [
            "store.db.bak-0114-202611261430",
            "store.db.bak-0114-202611261430.2"
        ]
    );
    assert!(applied(&path).is_empty());
}

#[test]
fn a_new_database_with_file_steps_is_not_backed_up() {
    let dir = tempfile::tempdir().unwrap();
    let path = database(&dir);

    run(&path, FROZEN, &[A, B]).unwrap();

    assert!(backups(&path).is_empty());
}

#[test]
fn an_applied_id_this_build_does_not_know_is_not_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = database(&dir);
    run(&path, FROZEN, &[A]).unwrap();

    run(&path, FROZEN, &[]).unwrap();
    run(&path, FROZEN, &[B]).unwrap();

    assert_eq!(applied(&path), [A.id, B.id]);
}

/// The unknown-id warnings logged while `path` is opened once with `files`.
fn warnings_on_open(path: &Path, files: &[FileMigration]) -> usize {
    UNKNOWN_ID_WARNINGS.with(|count| count.set(0));
    run(path, FROZEN, files).unwrap();
    UNKNOWN_ID_WARNINGS.with(std::cell::Cell::get)
}

#[test]
fn an_unknown_applied_id_is_logged_once_per_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = database(&dir);
    run(&path, FROZEN, &[A]).unwrap();

    assert_eq!(
        warnings_on_open(&path, &[B]),
        1,
        "an open that applies a step"
    );
    assert_eq!(warnings_on_open(&path, &[B]), 1, "an up-to-date open");
    assert_eq!(warnings_on_open(&path, &[A, B]), 0);

    let behind = dir.path().join("behind.db");
    run(&behind, &FROZEN[..1], &[A]).unwrap();
    assert_eq!(
        warnings_on_open(&behind, &[B]),
        1,
        "an open that also runs a frozen step"
    );
}

#[test]
fn an_up_to_date_database_is_not_written_to() {
    let dir = tempfile::tempdir().unwrap();
    let path = database(&dir);
    run(&path, FROZEN, &[A]).unwrap();
    let before = fs::read(&path).unwrap();

    run(&path, FROZEN, &[A]).unwrap();

    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn concurrent_openers_apply_each_file_step_once() {
    for round in 0..100 {
        let dir = tempfile::tempdir().unwrap();
        let path = database(&dir);
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE legacy (value TEXT);")
            .unwrap();

        let start = Barrier::new(2);
        let (first, second) = thread::scope(|scope| {
            let open = || {
                start.wait();
                run(&path, FROZEN, &[A, B])
            };
            let first = scope.spawn(open);
            let second = scope.spawn(open);
            (first.join().unwrap(), second.join().unwrap())
        });
        first.unwrap_or_else(|error| panic!("round {round}: first open failed: {error}"));
        second.unwrap_or_else(|error| panic!("round {round}: second open failed: {error}"));

        assert_eq!(applied(&path), [A.id, B.id], "round {round}");
        assert_eq!(backups(&path), ["store.db.bak-v0"], "round {round}");
    }
}
