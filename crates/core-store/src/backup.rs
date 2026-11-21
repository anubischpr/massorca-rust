//! Copies of an on-disk database, taken before each migration.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::error::StoreError;
use crate::store::BUSY_TIMEOUT;

/// Backups kept per database file; older ones are deleted after each new backup.
const KEPT_BACKUPS: usize = 5;

/// Whether the database has any schema object; a brand-new empty file has none.
pub(crate) fn holds_objects(connection: &Connection) -> Result<bool, StoreError> {
    Ok(
        connection.query_row("SELECT EXISTS (SELECT 1 FROM sqlite_master)", [], |row| {
            row.get(0)
        })?,
    )
}

/// Copies the database to `<file>.bak-v<version>` with `VACUUM INTO`, then prunes old backups.
/// `VACUUM INTO` gives a consistent snapshot even in WAL mode, where a plain file copy could miss frames.
/// The caller holds the migration's write lock, so no other migration can race this copy. It runs on its own
/// connection because `VACUUM INTO` cannot run inside the caller's transaction.
pub(crate) fn create_before_migration(file: &Path, version: i64) -> Result<(), StoreError> {
    let backup = backup_path(file, version)?;
    if backup.exists() {
        fs::remove_file(&backup)?;
    }
    let target = backup.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "store path is not valid UTF-8")
    })?;
    let snapshot = Connection::open(file)?;
    snapshot.busy_timeout(BUSY_TIMEOUT)?;
    snapshot.execute("VACUUM INTO ?1", [target])?;
    prune(file)
}

fn backup_path(file: &Path, version: i64) -> io::Result<PathBuf> {
    Ok(file.with_file_name(format!("{}.bak-v{version}", file_name(file)?)))
}

fn file_name(file: &Path) -> io::Result<&str> {
    file.file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "store path has no valid UTF-8 file name",
            )
        })
}

/// Keeps the newest `KEPT_BACKUPS` backups. Versions only grow, so the highest old version is the newest.
fn prune(file: &Path) -> Result<(), StoreError> {
    let prefix = format!("{}.bak-v", file_name(file)?);
    let directory = match file.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let mut backups: Vec<(u64, PathBuf)> = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(digits) = name
            .to_str()
            .and_then(|name| name.strip_prefix(prefix.as_str()))
        else {
            continue;
        };
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        if let Ok(version) = digits.parse::<u64>() {
            backups.push((version, entry.path()));
        }
    }
    backups.sort_unstable_by_key(|left| std::cmp::Reverse(left.0));
    for (_, path) in backups.into_iter().skip(KEPT_BACKUPS) {
        fs::remove_file(path)?;
    }
    Ok(())
}
