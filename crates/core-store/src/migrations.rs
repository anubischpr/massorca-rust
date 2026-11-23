use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

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
    // Bot Teams core (decision 0072): teams and their bots. A team's removal removes its bots; at most one bot is the lead.
    Migration {
        sql: r#"
CREATE TABLE teams (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    purpose TEXT NOT NULL,
    character TEXT NOT NULL,
    access TEXT NOT NULL,
    workspaces INTEGER NOT NULL CHECK(workspaces IN (0,1)),
    created_at INTEGER NOT NULL
);
CREATE TABLE bots (
    id TEXT PRIMARY KEY,
    team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    job TEXT NOT NULL,
    kind TEXT NOT NULL,
    model TEXT,
    effort TEXT,
    lead INTEGER NOT NULL CHECK(lead IN (0,1)),
    created_at INTEGER NOT NULL,
    UNIQUE(team_id, name)
);
CREATE UNIQUE INDEX bots_one_lead ON bots(team_id) WHERE lead = 1;
"#,
        foreign_keys_off: false,
    },
    // Team projects (decision 0072): each has its own Board and Brief; a team's removal removes them.
    Migration {
        sql: r#"
CREATE TABLE team_projects (
    id TEXT PRIMARY KEY,
    team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    goal TEXT NOT NULL,
    brief TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE(team_id, name)
);
"#,
        foreign_keys_off: false,
    },
    // Tasks (decision 0072). A task keeps its own lifecycle; the agent session it names is read from the agents domain.
    // A removed bot or project leaves its tasks without that link; a removed team takes its tasks with it.
    Migration {
        sql: r#"
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    project_id TEXT REFERENCES team_projects(id) ON DELETE SET NULL,
    bot_id TEXT REFERENCES bots(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    origin TEXT NOT NULL CHECK(origin IN ('room','cli')),
    session_id TEXT,
    outcome TEXT CHECK(outcome IN ('success','failure','cancelled','interrupted')),
    error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX tasks_by_team ON tasks(team_id, created_at);
CREATE INDEX tasks_by_bot ON tasks(bot_id);
CREATE INDEX tasks_by_project ON tasks(project_id);
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
    // Agent accounts (decision 0059): several credential homes per kind, one active per kind, and per-project choices.
    // Deleting an account clears its active mark and its choices; deleting a project clears its choices.
    Migration {
        sql: r#"
CREATE TABLE agent_accounts (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    label TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    identity TEXT,
    state TEXT NOT NULL CHECK(state IN ('signed_in','expired','needs_sign_in','not_set_up','migrated','conflict','unknown'))
);
CREATE INDEX agent_accounts_by_kind ON agent_accounts(kind, created_at);
CREATE TABLE agent_account_active (
    kind TEXT PRIMARY KEY,
    account TEXT NOT NULL REFERENCES agent_accounts(id) ON DELETE CASCADE
);
CREATE TABLE agent_account_choices (
    kind TEXT NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    account TEXT NOT NULL REFERENCES agent_accounts(id) ON DELETE CASCADE,
    PRIMARY KEY(kind, project_id)
);
CREATE INDEX agent_account_choices_by_account ON agent_account_choices(account);
"#,
        foreign_keys_off: false,
    },
    // Account origin (decision 0059): a sign-in is signed out when removed; an import or a migrated home is only deleted. Rows from
    // before this step default to 'import', so removing them never signs the person out of an original login.
    // Rows of the earlier step that came from 0056's home are marked `migrated`; the rest keep the 'import' default.
    Migration {
        sql: r#"
ALTER TABLE agent_accounts ADD COLUMN origin TEXT NOT NULL DEFAULT 'import' CHECK(origin IN ('sign_in','import','migrated'));
UPDATE agent_accounts SET origin = 'migrated' WHERE state = 'migrated';
"#,
        foreign_keys_off: false,
    },
    // Pending sign-ins and the record that a kind had an account (decision 0059). Only a row still pending is one the
    // startup sweep may decide; a kind that ever had an account never falls back to the agent's usual home.
    Migration {
        sql: r#"
ALTER TABLE agent_accounts ADD COLUMN sign_in_pending INTEGER NOT NULL DEFAULT 0 CHECK(sign_in_pending IN (0,1));
CREATE TABLE agent_account_history (kind TEXT PRIMARY KEY);
INSERT OR IGNORE INTO agent_account_history (kind) SELECT DISTINCT kind FROM agent_accounts;
CREATE TRIGGER agent_accounts_record_kind AFTER INSERT ON agent_accounts
BEGIN
    INSERT OR IGNORE INTO agent_account_history (kind) VALUES (NEW.kind);
END;
"#,
        foreign_keys_off: false,
    },
    // Whether an agent session's current or last run received vault secrets (decision 0071); never their names.
    Migration {
        sql: r#"
ALTER TABLE agent_sessions ADD COLUMN holds_secrets INTEGER NOT NULL DEFAULT 0 CHECK(holds_secrets IN (0,1));
"#,
        foreign_keys_off: false,
    },
    // Only a kept account counts as one the kind had (decision 0059): a row is recorded when it is inserted already decided
    // (an import or a migrated home) or when its pending sign-in is cleared. A sign-in deleted while still pending was
    // never kept and leaves no record. Rows recorded by the earlier trigger cannot tell a kept sign-in from a discarded
    // one, so they are left as they are.
    Migration {
        sql: r#"
DROP TRIGGER agent_accounts_record_kind;
CREATE TRIGGER agent_accounts_record_kind AFTER INSERT ON agent_accounts
WHEN NEW.sign_in_pending = 0
BEGIN
    INSERT OR IGNORE INTO agent_account_history (kind) VALUES (NEW.kind);
END;
CREATE TRIGGER agent_accounts_record_kept_sign_in AFTER UPDATE OF sign_in_pending ON agent_accounts
WHEN NEW.sign_in_pending = 0
BEGIN
    INSERT OR IGNORE INTO agent_account_history (kind) VALUES (NEW.kind);
END;
"#,
        foreign_keys_off: false,
    },
    // Team memory (decision 0072): lines a team or one of its bots keeps for later tasks. A bot-less line is the team's
    // scope. A team's or a bot's removal removes its lines.
    Migration {
        sql: r#"
CREATE TABLE team_memory (
    id TEXT PRIMARY KEY,
    team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    bot_id TEXT REFERENCES bots(id) ON DELETE CASCADE,
    text TEXT NOT NULL,
    source TEXT NOT NULL CHECK(source IN ('task','person')),
    task_id TEXT,
    created_at INTEGER NOT NULL
);
CREATE INDEX team_memory_by_scope ON team_memory(team_id, bot_id, created_at);
"#,
        foreign_keys_off: false,
    },
    // A team project's code source (decision 0072): the Workspaces project it links to. Removing that project clears the link.
    Migration {
        sql: r#"
ALTER TABLE team_projects ADD COLUMN source_project_id TEXT REFERENCES projects(id) ON DELETE SET NULL;
"#,
        foreign_keys_off: false,
    },
    // A team's folder workspace, made on its first task start; folder workspaces that let several agents write at once
    // (a team folder: no git index to corrupt, and shared files are the point); and the lookup from an agent session to
    // its task, the protocol code of a failed start beside its message, and the pending start attempt that keys a retry
    // to the same agent request (decision 0072 amendment).
    Migration {
        sql: r#"
ALTER TABLE teams ADD COLUMN workspace_id TEXT;
ALTER TABLE tasks ADD COLUMN error_code TEXT;
ALTER TABLE tasks ADD COLUMN start_attempt TEXT;
ALTER TABLE workspaces ADD COLUMN shared_writers INTEGER NOT NULL DEFAULT 0 CHECK(shared_writers IN (0,1));
CREATE INDEX tasks_session_id ON tasks (session_id);
"#,
        foreign_keys_off: false,
    },
    // Where a task's last start ran: its own worktree on a linked git project, a linked folder or the team folder, and
    // the worktree's branch, where the result is (decision 0072 amendment).
    Migration {
        sql: r#"
ALTER TABLE tasks ADD COLUMN workspace_id TEXT;
ALTER TABLE tasks ADD COLUMN branch TEXT;
"#,
        foreign_keys_off: false,
    },
    // Team routines (decision 0072 amendment): a schedule that starts a task for a bot on each occurrence. A team's
    // removal removes its routines and their runs; a bot's removal keeps the routine with no bot, and its runs record
    // why. A run keeps its task: removing the run or its routine never removes the task. A run whose `settled_at` is
    // NULL has not recorded its start's outcome yet; the next service start marks it interrupted.
    Migration {
        sql: r#"
CREATE TABLE team_routines (
    id TEXT PRIMARY KEY,
    team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    bot_id TEXT REFERENCES bots(id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    prompt TEXT NOT NULL,
    cadence TEXT NOT NULL CHECK(cadence IN ('daily','weekly','monthly','once')),
    time TEXT NOT NULL,
    weekday TEXT,
    month_day INTEGER CHECK(month_day IS NULL OR month_day BETWEEN 1 AND 28),
    run_date TEXT,
    zone TEXT,
    paused INTEGER NOT NULL DEFAULT 0 CHECK(paused IN (0,1)),
    starts_as_admin INTEGER NOT NULL CHECK(starts_as_admin IN (0,1)),
    next_run_at INTEGER,
    created_at INTEGER NOT NULL
);
CREATE INDEX team_routines_by_team ON team_routines (team_id, created_at);
CREATE INDEX team_routines_due ON team_routines (next_run_at);
CREATE TABLE team_routine_runs (
    id TEXT PRIMARY KEY,
    routine_id TEXT NOT NULL REFERENCES team_routines(id) ON DELETE CASCADE,
    scheduled_for INTEGER NOT NULL,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    error TEXT,
    error_code TEXT,
    settled_at INTEGER,
    created_at INTEGER NOT NULL
);
CREATE INDEX team_routine_runs_by_routine ON team_routine_runs (routine_id, scheduled_for);
"#,
        foreign_keys_off: false,
    },
    // An agent session's permission mode (decision 0083): the mode of its current or last run, whether launch arguments
    // confirmed it, where it came from (`vendor_default` or `chosen`), and the mode waiting for the next resume, kept
    // apart from the running one.
    Migration {
        sql: r#"
ALTER TABLE agent_sessions ADD COLUMN permission_mode TEXT;
ALTER TABLE agent_sessions ADD COLUMN mode_confirmed INTEGER NOT NULL DEFAULT 0 CHECK(mode_confirmed IN (0,1));
ALTER TABLE agent_sessions ADD COLUMN mode_source TEXT CHECK(mode_source IS NULL OR mode_source IN ('vendor_default','chosen'));
ALTER TABLE agent_sessions ADD COLUMN pending_permission_mode TEXT;
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
    migrate_observed(connection, file, steps, &mut |_| {})
}

/// Like `migrate`, calling `observe` once the migration lock is held and after each step.
pub(crate) fn migrate_observed(
    connection: &mut Connection,
    file: Option<&Path>,
    steps: &[Migration],
    observe: &mut dyn FnMut(&'static str),
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
    observe("store migration lock");
    if found == 0 && create_fresh(connection, file, steps)? {
        observe("store schema");
        return Ok(());
    }
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
        observe("store schema step");
    }
}

/// Builds the whole schema of a new, empty database file in one transaction: one commit and one sync instead of one per
/// step, which on a busy disk made a first start take longer than the client waits. Nothing is lost if it fails: the
/// database was empty. Returns `false`, changing nothing, for an in-memory database, one that already holds objects or
/// a version, or steps that need foreign keys switched off; those run one transaction per step, as an upgrade does.
fn create_fresh(
    connection: &mut Connection,
    file: Option<&Path>,
    steps: &[Migration],
) -> Result<bool, StoreError> {
    if file.is_none() || steps.iter().any(|step| step.foreign_keys_off) {
        return Ok(false);
    }
    create_in_one_transaction(connection, steps)
}

fn create_in_one_transaction(
    connection: &mut Connection,
    steps: &[Migration],
) -> Result<bool, StoreError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if user_version(&transaction)? != 0 || backup::holds_objects(&transaction)? {
        return Ok(false);
    }
    for step in steps {
        transaction.execute_batch(step.sql)?;
    }
    let version = i64::try_from(steps.len()).unwrap_or(i64::MAX);
    if let Some(table) = first_foreign_key_violation(&transaction)? {
        return Err(StoreError::ForeignKeyViolation { version, table });
    }
    transaction.execute_batch(&format!("PRAGMA user_version = {version}"))?;
    transaction.commit()?;
    Ok(true)
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

/// How long an open waits for another opener's upgrade before it gives up.
const UPGRADE_LOCK_WAIT: Duration = if cfg!(test) {
    Duration::from_secs(1)
} else {
    Duration::from_secs(30)
};

/// Takes the exclusive OS lock on `<file>.migrating`, waiting for another opener's upgrade to finish. The wait is
/// bounded, so a lock that is never released fails the open with a message instead of hanging it.
fn upgrade_lock(file: &Path) -> Result<File, StoreError> {
    let mut name = file.as_os_str().to_owned();
    name.push(".migrating");
    let path = PathBuf::from(name);
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)?;
    let deadline = Instant::now() + UPGRADE_LOCK_WAIT;
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(lock),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(TryLockError::WouldBlock) => {
                return Err(StoreError::Io(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!(
                        "{} stayed locked by another upgrade for {}s",
                        path.display(),
                        UPGRADE_LOCK_WAIT.as_secs()
                    ),
                )));
            }
            Err(TryLockError::Error(error)) => return Err(error.into()),
        }
    }
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

    #[test]
    fn an_upgrade_lock_that_is_never_released_fails_the_open_instead_of_hanging_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.db");
        let held = fs::File::create(dir.path().join("store.db.migrating")).unwrap();
        held.lock().unwrap();
        let mut connection = Connection::open(&path).unwrap();
        let started = std::time::Instant::now();
        let error = migrate(&mut connection, Some(&path), &STEPS[..1]).unwrap_err();
        assert!(
            matches!(&error, StoreError::Io(io) if io.kind() == std::io::ErrorKind::TimedOut),
            "{error:?}"
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        drop(held);
        migrate(&mut connection, Some(&path), &STEPS[..1]).unwrap();
    }

    fn schema(connection: &Connection) -> Vec<(String, String, Option<String>)> {
        let mut statement = connection
            .prepare("SELECT type, name, sql FROM sqlite_master ORDER BY type, name")
            .unwrap();
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn a_new_database_gets_the_same_schema_in_one_transaction_as_step_by_step() {
        let dir = tempfile::tempdir().unwrap();
        let fresh_path = dir.path().join("fresh.db");
        let mut fresh = Connection::open(&fresh_path).unwrap();
        migrate(&mut fresh, Some(&fresh_path), super::MIGRATIONS).unwrap();

        let stepped_path = dir.path().join("stepped.db");
        let mut stepped = Connection::open(&stepped_path).unwrap();
        // One step first: the database then holds objects, so the rest runs step by step as an upgrade does.
        migrate(&mut stepped, Some(&stepped_path), &super::MIGRATIONS[..1]).unwrap();
        migrate(&mut stepped, Some(&stepped_path), super::MIGRATIONS).unwrap();

        assert_eq!(schema(&fresh), schema(&stepped));
        let version = |connection: &Connection| -> i64 {
            connection
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap()
        };
        assert_eq!(
            version(&fresh),
            i64::try_from(super::MIGRATIONS.len()).unwrap()
        );
        assert_eq!(version(&fresh), version(&stepped));
        assert!(
            !fs::read_dir(dir.path()).unwrap().any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("fresh.db.bak")),
            "a new database has nothing to back up"
        );
    }
}
