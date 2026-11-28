//! Creating a store file that does not exist yet (decision 0184). The schema is built under a temporary name with
//! every sync switched off, the file is synced once, and only then does it appear under its real name.

use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::Connection;
use tracing::warn;

use crate::error::StoreError;
use crate::migration_files::FileMigration;
use crate::migrations::{self, Migration};

const TEMP_MARKER: &str = ".creating-";

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Builds the store at `path` when nothing is there. Returns once `path` is a complete database or was already
/// present; a failed build leaves no file at `path`.
pub(crate) fn create_if_missing(
    path: &Path,
    steps: &[Migration],
    files: &[FileMigration],
    observe: &mut dyn FnMut(&'static str),
) -> Result<(), StoreError> {
    if path.try_exists()? {
        return Ok(());
    }
    // Every builder holds this lock while its temporary file exists, so any temporary file seen under it is stale.
    let _lock = migrations::upgrade_lock(path)?;
    observe("store migration lock");
    if path.try_exists()? {
        return Ok(());
    }
    remove_stale_temps(path)?;
    let temp = temp_path(path);
    let created = build(&temp, steps, files, observe).and_then(|()| place(&temp, path));
    if created.is_err() {
        remove_if_present(&temp);
    }
    created
}

fn build(
    temp: &Path,
    steps: &[Migration],
    files: &[FileMigration],
    observe: &mut dyn FnMut(&'static str),
) -> Result<(), StoreError> {
    let mut connection = Connection::open(temp)?;
    // Safe without syncs or an on-disk journal: until the rename nobody can see this file, and a crash only leaves a
    // stale temporary file. The in-memory journal still lets a failing file step roll back to its savepoint.
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = OFF;")?;
    connection.pragma_update(None, "journal_mode", "MEMORY")?;
    migrations::build_new_observed(&mut connection, steps, files, observe)?;
    connection.close().map_err(|(_, error)| error)?;
    OpenOptions::new().write(true).open(temp)?.sync_all()?;
    Ok(())
}

/// Gives the synced `temp` the name `path` without replacing a file another opener put there, then drops `temp`.
fn place(temp: &Path, path: &Path) -> Result<(), StoreError> {
    match fs::hard_link(temp, path) {
        Ok(()) => {}
        // An opener that skips the lock (an older build) got there first; its database is kept.
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        // No hard links on this file system; under the lock a plain rename only races openers that skip it.
        Err(_) if !path.try_exists()? => fs::rename(temp, path)?,
        Err(error) => return Err(error.into()),
    }
    remove_if_present(temp);
    sync_parent(path)
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> Result<(), StoreError> {
    let parent = parent_dir(path);
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

// Windows cannot sync a directory handle; NTFS journals the new name itself.
#[cfg(not(unix))]
fn sync_parent(_path: &Path) -> Result<(), StoreError> {
    Ok(())
}

fn parent_dir(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

fn temp_prefix(path: &Path) -> Option<OsString> {
    let mut prefix = path.file_name()?.to_owned();
    prefix.push(TEMP_MARKER);
    Some(prefix)
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = temp_prefix(path).unwrap_or_else(|| OsString::from(TEMP_MARKER));
    let unique = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    name.push(format!("{}-{unique}", std::process::id()));
    parent_dir(path).join(name)
}

fn remove_stale_temps(path: &Path) -> Result<(), StoreError> {
    let Some(prefix) = temp_prefix(path) else {
        return Ok(());
    };
    for entry in fs::read_dir(parent_dir(path))? {
        let entry = entry?;
        if starts_with(&entry.file_name(), &prefix) {
            match fs::remove_file(entry.path()) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
                _ => {}
            }
        }
    }
    Ok(())
}

fn starts_with(name: &OsStr, prefix: &OsStr) -> bool {
    name.as_encoded_bytes()
        .starts_with(prefix.as_encoded_bytes())
}

fn remove_if_present(temp: &Path) {
    match fs::remove_file(temp) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            warn!(path = %temp.display(), %error, "could not remove a temporary store file");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use rusqlite::Connection;

    use super::{TEMP_MARKER, create_if_missing, place};
    use crate::migration_files::{FILE_MIGRATIONS, FileMigration};
    use crate::migrations::{MIGRATIONS, Migration};
    use crate::store::Store;

    fn temp_files(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(TEMP_MARKER))
            .collect()
    }

    /// Every schema object, every recorded migration file and `user_version`.
    type Schema = (Vec<(String, String, Option<String>)>, Vec<String>, i64);

    fn schema(path: &Path) -> Schema {
        let connection = Connection::open(path).unwrap();
        let mut objects = connection
            .prepare("SELECT type, name, sql FROM sqlite_master ORDER BY type, name")
            .unwrap();
        let objects = objects
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        let mut ids = connection
            .prepare("SELECT id FROM store_migrations ORDER BY id")
            .unwrap();
        let ids = ids
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        let version = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        (objects, ids, version)
    }

    #[test]
    fn a_new_store_matches_one_built_in_place_and_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let fresh = dir.path().join("fresh.db");
        Store::open(&fresh).unwrap();
        // An empty file that already exists still takes the in-place path.
        let in_place = dir.path().join("in-place.db");
        fs::File::create(&in_place).unwrap();
        Store::open(&in_place).unwrap();

        let (objects, ids, version) = schema(&fresh);
        assert_eq!((objects.clone(), ids.clone(), version), schema(&in_place));
        assert_eq!(ids.len(), FILE_MIGRATIONS.len());
        assert_eq!(usize::try_from(version).unwrap(), MIGRATIONS.len());
        assert!(temp_files(dir.path()).is_empty());
        let mode: String = Connection::open(&fresh)
            .unwrap()
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }

    #[test]
    fn placing_never_replaces_a_file_that_appeared_meanwhile() {
        let dir = tempfile::tempdir().unwrap();
        let temp = dir.path().join(format!("store.db{TEMP_MARKER}1-0"));
        let target = dir.path().join("store.db");
        fs::write(&temp, b"built").unwrap();
        fs::write(&target, b"theirs").unwrap();
        place(&temp, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"theirs");
        assert!(!temp.exists());
    }

    #[test]
    fn an_existing_store_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("store.db");
        fs::write(&target, b"not touched").unwrap();
        create_if_missing(&target, MIGRATIONS, FILE_MIGRATIONS, &mut |_| {}).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"not touched");
        assert!(temp_files(dir.path()).is_empty());
    }

    #[test]
    fn a_temporary_file_left_by_a_crash_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let stale = dir.path().join(format!("store.db{TEMP_MARKER}999-0"));
        fs::write(&stale, b"half a database").unwrap();
        let other = dir.path().join(format!("other.db{TEMP_MARKER}999-0"));
        fs::write(&other, b"another store's").unwrap();
        Store::open(dir.path().join("store.db")).unwrap();
        assert!(!stale.exists());
        assert!(
            other.exists(),
            "only this store's temporary files are removed"
        );
    }

    #[test]
    fn a_failed_build_leaves_no_store_and_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("store.db");
        let broken = [Migration {
            sql: "CREATE TABLE a (x INTEGER); CREATE TABLE broken (",
            foreign_keys_off: false,
        }];
        assert!(create_if_missing(&target, &broken, &[], &mut |_| {}).is_err());
        let failing_file = [FileMigration {
            id: "0184-000000000000",
            sql: "INSERT INTO missing VALUES (1)",
            foreign_keys_off: false,
        }];
        let steps = [Migration {
            sql: "CREATE TABLE a (x INTEGER);",
            foreign_keys_off: false,
        }];
        assert!(create_if_missing(&target, &steps, &failing_file, &mut |_| {}).is_err());
        assert!(!target.exists());
        assert!(temp_files(dir.path()).is_empty());
    }
}
