use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, TransactionBehavior};

use crate::backup;
use crate::error::StoreError;

/// One schema step. A table rebuild sets `foreign_keys_off`: enforcement is off around its transaction, which is rolled
/// back if it leaves a foreign key violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub sql: &'static str,
    pub foreign_keys_off: bool,
}

/// Schema steps in order. The number applied is the database's `user_version`; shipped steps are never edited.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        sql: r#"
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
"#,
        foreign_keys_off: false,
    },
    // Agent sessions (decision 0051). A workspace's removal removes its sessions. Forgetting a parent keeps its
    // children, which become roots: they are sessions of their own in their own workspaces, and deleting them with
    // the parent's record would lose their results.
    Migration {
        sql: r#"
CREATE TABLE agent_sessions (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    parent_id TEXT REFERENCES agent_sessions(id) ON DELETE SET NULL,
    title TEXT,
    read_only INTEGER NOT NULL,
    agent_ref TEXT,
    terminal_id TEXT,
    request_id TEXT UNIQUE,
    created_at INTEGER NOT NULL,
    exit_code INTEGER,
    exit_signal TEXT,
    exit_reason TEXT CHECK(exit_reason IN ('exited','killed','service_stopped')),
    exited_at INTEGER,
    activity TEXT CHECK(activity IN ('working','waiting','idle','done')),
    activity_outcome TEXT CHECK(activity_outcome IN ('success','failure','cancelled','interrupted')),
    activity_source TEXT CHECK(activity_source IN ('hook','process','output')),
    activity_at INTEGER,
    result TEXT,
    brought_back_at INTEGER
);
CREATE INDEX agent_sessions_by_workspace ON agent_sessions(workspace_id, created_at);
CREATE INDEX agent_sessions_by_parent ON agent_sessions(parent_id);
"#,
        foreign_keys_off: false,
    },
    Migration {
        // Paired network devices (decision 0041). Only the hash of a device's public key is kept.
        sql: r#"
CREATE TABLE devices (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    key_pin TEXT NOT NULL UNIQUE,
    scopes TEXT NOT NULL,
    paired_at INTEGER NOT NULL,
    last_seen_at INTEGER,
    revoked_at INTEGER
);
"#,
        foreign_keys_off: false,
    },
    // Secret vault (decision 0071): ciphertext only; names and scopes in clear.
    Migration {
        sql: r#"
CREATE TABLE secrets (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL,
    name TEXT NOT NULL,
    nonce BLOB NOT NULL,
    ciphertext BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    rotated_at INTEGER NOT NULL,
    used_at INTEGER,
    UNIQUE(scope, name)
);
CREATE TABLE secret_links (
    scope TEXT NOT NULL,
    name TEXT NOT NULL,
    secret_id TEXT NOT NULL REFERENCES secrets(id) ON DELETE CASCADE,
    linked_at INTEGER NOT NULL,
    PRIMARY KEY(scope, name)
);
CREATE INDEX secret_links_by_secret ON secret_links(secret_id);
CREATE TABLE secret_requests (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL,
    name TEXT NOT NULL,
    reason TEXT NOT NULL,
    session TEXT,
    state TEXT NOT NULL CHECK(state IN ('pending','stored','linked','declined','failed')),
    created_at INTEGER NOT NULL,
    resolved_at INTEGER
);
"#,
        foreign_keys_off: false,
    },
    // The vault key check, so a planted key is refused, and who asked for each request, for its limits (0071).
    Migration {
        sql: r#"
CREATE TABLE secret_key_check (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    nonce BLOB NOT NULL,
    sealed BLOB NOT NULL,
    created_at INTEGER NOT NULL
);
ALTER TABLE secret_requests ADD COLUMN requester TEXT;
CREATE INDEX secret_requests_by_requester ON secret_requests(requester, created_at);
"#,
        foreign_keys_off: false,
    },
];

/// Applies every step newer than the database's `user_version`. A newer database is refused before anything is written;
/// `file` is `Some` for on-disk databases, which are backed up before their first migration.
/// The whole upgrade of a file runs under an OS lock, and each step re-reads `user_version` under the write lock, so
/// concurrent openers migrate a file once and back it up once.
pub(crate) fn migrate(
    connection: &mut Connection,
    file: Option<&Path>,
    steps: &[Migration],
) -> Result<(), StoreError> {
    let supported = i64::try_from(steps.len()).unwrap_or(i64::MAX);
    // An up-to-date or newer database is answered without the migration lock, so it is never written to.
    let found = user_version(connection)?;
    if found > supported {
        return Err(StoreError::NewerSchema { found, supported });
    }
    if found == supported {
        return Ok(());
    }
    // Held across every step: SQLite's write lock is released between steps, and a second opener that took it there
    // would back up the half-migrated file again.
    let _upgrade = file.map(upgrade_lock).transpose()?;
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

/// Runs one step, with foreign keys switched off around its transaction when the step asks for that.
fn apply_step(
    connection: &mut Connection,
    file: Option<&Path>,
    found: i64,
    index: usize,
    step: &Migration,
    backed_up: &mut bool,
) -> Result<(), StoreError> {
    if !step.foreign_keys_off {
        return run_step(connection, file, found, index, step, backed_up);
    }
    // `PRAGMA foreign_keys` does nothing inside a transaction, so it is set before the step's transaction and
    // restored after it, whether the step committed or was rolled back.
    let enforced = foreign_keys_enforced(connection)?;
    connection.pragma_update(None, "foreign_keys", false)?;
    let result = run_step(connection, file, found, index, step, backed_up);
    let restored = connection.pragma_update(None, "foreign_keys", enforced);
    result.and(restored.map_err(StoreError::from))
}

/// Runs the step in one immediate transaction if `user_version` still reads `found`. Otherwise another process has
/// moved it on, and the caller reads it again.
fn run_step(
    connection: &mut Connection,
    file: Option<&Path>,
    found: i64,
    index: usize,
    step: &Migration,
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
    transaction.execute_batch(step.sql)?;
    if step.foreign_keys_off
        && let Some(table) = first_foreign_key_violation(&transaction)?
    {
        return Err(StoreError::ForeignKeyViolation {
            version: i64::try_from(index + 1).unwrap_or(i64::MAX),
            table,
        });
    }
    transaction.execute_batch(&format!("PRAGMA user_version = {}", index + 1))?;
    transaction.commit()?;
    Ok(())
}

/// Takes the exclusive OS lock on `<file>.migrating`, waiting for another opener's upgrade to finish.
fn upgrade_lock(file: &Path) -> Result<File, StoreError> {
    let mut name = file.as_os_str().to_owned();
    name.push(".migrating");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(PathBuf::from(name))?;
    lock.lock()?;
    Ok(lock)
}

fn user_version(connection: &Connection) -> rusqlite::Result<i64> {
    connection.query_row("PRAGMA user_version", [], |row| row.get(0))
}

fn foreign_keys_enforced(connection: &Connection) -> rusqlite::Result<bool> {
    connection.query_row("PRAGMA foreign_keys", [], |row| row.get(0))
}

/// The table of the first row that breaks a foreign key, if any.
fn first_foreign_key_violation(connection: &Connection) -> rusqlite::Result<Option<String>> {
    let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
    let mut rows = statement.query([])?;
    let first = rows.next()?;
    first.map(|row| row.get(0)).transpose()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rusqlite::Connection;

    use super::{Migration, migrate};
    use crate::error::StoreError;

    const fn step(sql: &'static str) -> Migration {
        Migration {
            sql,
            foreign_keys_off: false,
        }
    }

    const STEPS: &[Migration] = &[
        step("CREATE TABLE step_1 (x INTEGER);"),
        step("CREATE TABLE step_2 (x INTEGER);"),
        step("CREATE TABLE step_3 (x INTEGER);"),
        step("CREATE TABLE step_4 (x INTEGER);"),
        step("CREATE TABLE step_5 (x INTEGER);"),
        step("CREATE TABLE step_6 (x INTEGER);"),
        step("CREATE TABLE step_7 (x INTEGER);"),
    ];

    const PARENT_AND_CHILD: Migration = step(
        "CREATE TABLE parent (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
         CREATE TABLE child (id INTEGER PRIMARY KEY, parent_id INTEGER NOT NULL REFERENCES parent (id));
         INSERT INTO parent VALUES (1, 'kept');
         INSERT INTO child VALUES (10, 1);",
    );
    /// The SQLite table-rebuild procedure: create the new table, copy, drop the old one, rename.
    const REBUILD_PARENT: Migration = Migration {
        sql: "CREATE TABLE parent_new (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
              INSERT INTO parent_new SELECT id, name FROM parent;
              DROP TABLE parent;
              ALTER TABLE parent_new RENAME TO parent;",
        foreign_keys_off: true,
    };
    const DELETE_PARENT: Migration = Migration {
        sql: "DELETE FROM parent;",
        foreign_keys_off: true,
    };
    const ORPHAN_CHILD: Migration = step("INSERT INTO child VALUES (11, 99);");

    /// A connection set up the way `Store` sets up its own: foreign keys enforced.
    fn enforcing_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", true)
            .unwrap();
        connection
    }

    fn version(connection: &Connection) -> i64 {
        connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap()
    }

    fn foreign_keys_enforced(connection: &Connection) -> bool {
        connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap()
    }

    fn row_count(connection: &Connection, table: &str) -> i64 {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

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

    #[test]
    fn foreign_keys_off_step_can_rebuild_a_table_that_rows_reference() {
        let mut connection = enforcing_connection();
        migrate(&mut connection, None, &[PARENT_AND_CHILD, REBUILD_PARENT]).unwrap();

        assert_eq!(version(&connection), 2);
        assert!(foreign_keys_enforced(&connection));
        assert_eq!(row_count(&connection, "parent"), 1);
        assert_eq!(row_count(&connection, "child"), 1);
    }

    #[test]
    fn the_same_rebuild_is_refused_while_foreign_keys_stay_on() {
        let mut connection = enforcing_connection();
        let rebuild_with_enforcement = Migration {
            foreign_keys_off: false,
            ..REBUILD_PARENT
        };
        let error = migrate(
            &mut connection,
            None,
            &[PARENT_AND_CHILD, rebuild_with_enforcement],
        )
        .unwrap_err();

        assert!(matches!(error, StoreError::Constraint { .. }), "{error:?}");
        assert_eq!(version(&connection), 1);
    }

    #[test]
    fn foreign_keys_off_step_that_leaves_a_violation_is_rolled_back() {
        let mut connection = enforcing_connection();
        migrate(&mut connection, None, &[PARENT_AND_CHILD]).unwrap();

        let error = migrate(&mut connection, None, &[PARENT_AND_CHILD, DELETE_PARENT]).unwrap_err();

        assert!(
            matches!(
                error,
                StoreError::ForeignKeyViolation { version: 2, ref table } if table == "child"
            ),
            "{error:?}"
        );
        assert_eq!(version(&connection), 1);
        assert_eq!(row_count(&connection, "parent"), 1);
        assert!(foreign_keys_enforced(&connection));
    }

    #[test]
    fn ordinary_steps_keep_foreign_keys_enforced() {
        let mut connection = enforcing_connection();
        migrate(&mut connection, None, &[PARENT_AND_CHILD]).unwrap();

        let error = migrate(&mut connection, None, &[PARENT_AND_CHILD, ORPHAN_CHILD]).unwrap_err();

        assert!(matches!(error, StoreError::Constraint { .. }), "{error:?}");
        assert_eq!(version(&connection), 1);
        assert_eq!(row_count(&connection, "child"), 1);
    }
}
