//! Copies of an on-disk database, taken before each migration.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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

/// What a backup is named after: the frozen version it was taken at, or the file step it comes before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackupLabel<'a> {
    Version(i64),
    Step(&'a str),
}

/// Copies the database to `<file>.bak-<label>` with `VACUUM INTO`, then prunes old backups.
/// `VACUUM INTO` gives a consistent snapshot even in WAL mode, where a plain file copy could miss frames.
/// The caller holds the migration's write lock, so no other migration can race this copy. It runs on its own
/// connection because `VACUUM INTO` cannot run inside the caller's transaction.
pub(crate) fn create_before_migration(file: &Path, label: BackupLabel<'_>) -> Result<(), StoreError> {
    let backup = unused_backup_path(file, label)?;
    let target = backup.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "store path is not valid UTF-8")
    })?;
    let snapshot = Connection::open(file)?;
    snapshot.busy_timeout(BUSY_TIMEOUT)?;
    snapshot.execute("VACUUM INTO ?1", [target])?;
    prune(file)
}

/// `<file>.bak-<label>`, or `.2`, `.3`... after it when that name is taken: an existing backup is never replaced.
fn unused_backup_path(file: &Path, label: BackupLabel<'_>) -> io::Result<PathBuf> {
    let name = file_name(file)?;
    let base = match label {
        BackupLabel::Version(version) => format!("{name}.bak-v{version}"),
        BackupLabel::Step(id) => format!("{name}.bak-{id}"),
    };
    let first = file.with_file_name(&base);
    if !first.exists() {
        return Ok(first);
    }
    (2..)
        .map(|copy| file.with_file_name(format!("{base}.{copy}")))
        .find(|path| !path.exists())
        .ok_or_else(|| io::Error::other("no free backup name"))
}

/// `v<version>` or a file step's `<block>-<yyyymmddhhmm>`, optionally followed by `.<copy>`.
fn is_backup_label(label: &str) -> bool {
    let (base, copy) = match label.split_once('.') {
        Some((base, copy)) => (base, Some(copy)),
        None => (label, None),
    };
    let copy_ok = copy.is_none_or(|copy| !copy.is_empty() && copy.bytes().all(|byte| byte.is_ascii_digit()));
    let version_ok = base
        .strip_prefix('v')
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()));
    copy_ok && (version_ok || is_step_id(base))
}

/// `<block>-<yyyymmddhhmm>`: four digits, a dash, then twelve digits.
fn is_step_id(base: &str) -> bool {
    base.len() == 17
        && base
            .bytes()
            .enumerate()
            .all(|(index, byte)| if index == 4 { byte == b'-' } else { byte.is_ascii_digit() })
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

/// Keeps the newest `KEPT_BACKUPS` backups of this database, by modification time, whichever naming scheme they use.
fn prune(file: &Path) -> Result<(), StoreError> {
    let prefix = format!("{}.bak-", file_name(file)?);
    let directory = match file.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let mut backups: Vec<(SystemTime, String, PathBuf)> = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        let Some(label) = name.strip_prefix(prefix.as_str()) else {
            continue;
        };
        if is_backup_label(label) {
            backups.push((entry.metadata()?.modified()?, name.to_owned(), entry.path()));
        }
    }
    backups.sort_unstable_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
    for (_, _, path) in backups.into_iter().skip(KEPT_BACKUPS) {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    use super::prune;

    /// Writes a backup named `name` whose modification time is `age` seconds after the epoch.
    fn backup(dir: &Path, name: &str, age: u64) {
        let path = dir.join(name);
        fs::write(&path, name).unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(age))
            .unwrap();
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn prune_keeps_the_five_newest_backups_of_both_naming_schemes() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("store.db");
        backup(dir.path(), "store.db.bak-v0", 1);
        backup(dir.path(), "store.db.bak-v1", 2);
        backup(dir.path(), "store.db.bak-0110-202611261430", 3);
        backup(dir.path(), "store.db.bak-0110-202611261430.2", 4);
        backup(dir.path(), "store.db.bak-0111-202611261430", 5);
        backup(dir.path(), "store.db.bak-0112-202611261430", 6);
        // Not a backup of this database, or not named in either scheme: never pruned.
        backup(dir.path(), "store.db.bak-notes", 0);
        backup(dir.path(), "store.db.bak-v2x", 0);
        backup(dir.path(), "store.db.bak-0112-202611261430.x", 0);
        backup(dir.path(), "other.db.bak-v0", 0);

        prune(&file).unwrap();

        assert_eq!(
            names(dir.path()),
            [
                "other.db.bak-v0",
                "store.db.bak-0110-202611261430",
                "store.db.bak-0110-202611261430.2",
                "store.db.bak-0111-202611261430",
                "store.db.bak-0112-202611261430",
                "store.db.bak-0112-202611261430.x",
                "store.db.bak-notes",
                "store.db.bak-v1",
                "store.db.bak-v2x",
            ]
        );
    }
}
