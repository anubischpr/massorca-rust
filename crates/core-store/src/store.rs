use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use rusqlite::{Connection, OptionalExtension, Params, Row};

use crate::error::StoreError;
use crate::migration_files::FILE_MIGRATIONS;
use crate::migrations::{self, MIGRATIONS};

pub(crate) const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Durable MassOrca state. Cheap to clone; methods are synchronous, so async callers use `spawn_blocking`.
#[derive(Clone)]
pub struct Store {
    connection: Arc<Mutex<Connection>>,
}

impl fmt::Debug for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Store").finish_non_exhaustive()
    }
}

impl Store {
    /// Opens the database at `path`, creating it if needed, and migrates it to the latest schema.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Self::open_observed(path, |_| {})
    }

    /// Like `open`, calling `step` as each part of the open ends, so a slow open says which part took the time.
    pub fn open_observed(
        path: impl AsRef<Path>,
        mut step: impl FnMut(&'static str),
    ) -> Result<Self, StoreError> {
        let path = path.as_ref();
        let mut connection = Connection::open(path)?;
        configure(&connection)?;
        step("store file");
        // WAL is switched on only after migration, so a refused newer database is never modified.
        migrations::migrate_observed(
            &mut connection,
            Some(path),
            MIGRATIONS,
            FILE_MIGRATIONS,
            &mut step,
        )?;
        enable_wal(&connection)?;
        step("store wal");
        Ok(Self::from_connection(connection))
    }

    /// An in-memory database with the latest schema; nothing is persisted or backed up.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let mut connection = Connection::open_in_memory()?;
        configure(&connection)?;
        migrations::migrate_observed(
            &mut connection,
            None,
            MIGRATIONS,
            FILE_MIGRATIONS,
            &mut |_| {},
        )?;
        Ok(Self::from_connection(connection))
    }

    fn from_connection(connection: Connection) -> Self {
        Self {
            connection: Arc::new(Mutex::new(connection)),
        }
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Connection> {
        // A panic while holding the lock leaves no half-written row: each write is one statement or a transaction that rolls back.
        self.connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn query_optional<T>(
        &self,
        sql: &str,
        params: impl Params,
        map: impl FnOnce(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Option<T>, StoreError> {
        Ok(self.lock().query_row(sql, params, map).optional()?)
    }

    pub(crate) fn query_all<T>(
        &self,
        sql: &str,
        params: impl Params,
        map: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>, StoreError> {
        let connection = self.lock();
        let mut statement = connection.prepare(sql)?;
        let rows = statement.query_map(params, map)?;
        Ok(rows.collect::<rusqlite::Result<Vec<T>>>()?)
    }
}

/// Connection settings that never write to the file, so refusing a newer database leaves it untouched.
fn configure(connection: &Connection) -> Result<(), StoreError> {
    connection.busy_timeout(BUSY_TIMEOUT)?;
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = NORMAL;")?;
    Ok(())
}

/// Switches the file to WAL, retrying `SQLITE_BUSY` for up to the busy timeout. The switch takes a read lock and then
/// promotes it to the write lock; while another connection holds the write lock the promotion fails at once and SQLite
/// skips the busy handler to avoid a deadlock, so the waiting has to happen here.
fn enable_wal(connection: &Connection) -> Result<(), StoreError> {
    let deadline = Instant::now() + BUSY_TIMEOUT;
    let mut pause = Duration::from_millis(1);
    loop {
        match connection.pragma_update(None, "journal_mode", "WAL") {
            Ok(()) => return Ok(()),
            Err(error) if is_busy(&error) && Instant::now() < deadline => {
                thread::sleep(pause);
                pause = (pause * 2).min(Duration::from_millis(50));
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn is_busy(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(failure, _) if failure.code == rusqlite::ErrorCode::DatabaseBusy)
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use rusqlite::Connection;

    use super::{BUSY_TIMEOUT, enable_wal};

    #[test]
    fn wal_switch_waits_for_a_writer_that_holds_the_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.sqlite3");
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE t (x INTEGER);")
            .unwrap();

        // `holder` keeps the write lock briefly. While it is held SQLite fails the switch at once, without the
        // busy timeout, so the switch has to retry rather than return that error.
        let holder = Connection::open(&path).unwrap();
        holder.execute_batch("BEGIN IMMEDIATE").unwrap();
        let release = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            holder.execute_batch("COMMIT").unwrap();
        });

        let switcher = Connection::open(&path).unwrap();
        switcher.busy_timeout(BUSY_TIMEOUT).unwrap();
        enable_wal(&switcher).unwrap();
        release.join().unwrap();

        let mode: String = Connection::open(&path)
            .unwrap()
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }

    #[test]
    fn an_observed_open_reports_each_part_in_order_and_only_the_file_and_wal_once_created() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.db");
        let mut parts = Vec::new();
        super::Store::open_observed(&path, |part| parts.push(part)).unwrap();
        // The frozen list is built in one step; each file step is its own step after it.
        let mut expected = vec!["store file", "store migration lock", "store schema"];
        expected.extend(
            crate::migration_files::FILE_MIGRATIONS
                .iter()
                .map(|_| "store schema step"),
        );
        expected.push("store wal");
        assert_eq!(
            parts, expected,
            "a new database is built in one step, then one per file step"
        );
        let mut parts = Vec::new();
        super::Store::open_observed(&path, |part| parts.push(part)).unwrap();
        assert_eq!(parts, ["store file", "store wal"]);
    }
}
