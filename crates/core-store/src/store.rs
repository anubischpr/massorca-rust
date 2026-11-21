use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, Params, Row};

use crate::error::StoreError;
use crate::migrations::{self, MIGRATIONS};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

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
        let path = path.as_ref();
        let mut connection = Connection::open(path)?;
        configure(&connection)?;
        // WAL is switched on only after migration, so a refused newer database is never modified.
        migrations::migrate(&mut connection, Some(path), MIGRATIONS)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        Ok(Self::from_connection(connection))
    }

    /// An in-memory database with the latest schema; nothing is persisted or backed up.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let mut connection = Connection::open_in_memory()?;
        configure(&connection)?;
        migrations::migrate(&mut connection, None, MIGRATIONS)?;
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
