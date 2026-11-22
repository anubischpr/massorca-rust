//! A folder of the profile held open without following links, and the entries it creates, writes and removes
//! (ADR 0073). On Windows the folder is pinned, so it cannot be renamed or replaced while it is held, and its entries are
//! reached through its path. On Unix entries are created, opened and renamed relative to the folder's descriptor
//! (`openat`, `mkdirat`, `renameat`, `unlinkat`). A removed folder is first renamed through an open handle; the
//! renamed folder is then deleted by path inside the held folder with `fs::remove_dir_all`, which deletes links
//! themselves and never follows them.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::root_access::{RootAccessError, platform_create_private_dir};

/// Names starting with this are folders being removed; `list`-style readers skip them and `sweep_leftovers` deletes them.
pub const TOMBSTONE_PREFIX: &str = ".removing-";

/// Names starting with this are files being written.
const TEMP_PREFIX: &str = ".writing-";

/// How often a leftover is deleted before it is left for the next sweep; a virus scanner can hold a file briefly.
const DELETE_ATTEMPTS: u32 = 3;

/// Whether `name` is something a write or a removal left behind, never a real entry.
pub fn is_leftover(name: &str) -> bool {
    name.starts_with(TOMBSTONE_PREFIX) || name.starts_with(TEMP_PREFIX)
}

#[derive(Debug)]
pub struct PrivateDir {
    path: PathBuf,
    #[cfg(windows)]
    _pin: crate::windows_dir_access::PinnedDir,
    #[cfg(unix)]
    dir: fs::File,
}

/// Creates `dir` private to its owner when it is missing, then opens it without following links. Refuses a link or a
/// file there with `UnsafeLocation` and a folder another user owns with `ForeignEntries`. A folder others could change
/// is refused with `WritableByOthers`: its content cannot be trusted. On Windows a folder others could read is refused
/// with `ReadableByOthers`, and one only the accepted owners and Administrators can reach is made owner-only; on Unix a
/// folder that is only readable by others is set `0700`. Run `secure_root` first, so the folder it sits in is checked.
pub fn open_private_dir(dir: &Path) -> Result<PrivateDir, RootAccessError> {
    platform_create_private_dir(dir)?;
    open_existing(dir)
}

#[cfg(windows)]
fn open_existing(dir: &Path) -> Result<PrivateDir, RootAccessError> {
    use crate::windows_profile_root::{FolderAccess, folder_access, make_owner_only};

    let pin = crate::windows_dir_access::PinnedDir::open(dir)
        .map_err(|error| refused_open(dir, error))?;
    let paths = || vec![dir.to_path_buf()];
    match folder_access(dir)? {
        FolderAccess::Private => {}
        FolderAccess::OwnersOnly => make_owner_only(dir)?,
        FolderAccess::WritableByOthers => {
            return Err(RootAccessError::WritableByOthers { paths: paths() });
        }
        FolderAccess::ReadableByOthers => {
            return Err(RootAccessError::ReadableByOthers { paths: paths() });
        }
    }
    Ok(PrivateDir {
        path: dir.to_path_buf(),
        _pin: pin,
    })
}

#[cfg(unix)]
fn open_existing(dir: &Path) -> Result<PrivateDir, RootAccessError> {
    let file = crate::unix_dir_fd::open_dir(None, dir).map_err(|error| refused_open(dir, error))?;
    Ok(PrivateDir {
        path: dir.to_path_buf(),
        dir: crate::unix_dir_fd::checked(file, dir)?,
    })
}

#[cfg(not(any(unix, windows)))]
fn open_existing(dir: &Path) -> Result<PrivateDir, RootAccessError> {
    Ok(PrivateDir {
        path: dir.to_path_buf(),
    })
}

impl PrivateDir {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Creates the folder `name` inside this one when it is missing, private, and opens it the same way.
    pub fn child(&self, name: &str) -> Result<PrivateDir, RootAccessError> {
        check_entry_name(name)?;
        let path = self.path.join(name);
        #[cfg(unix)]
        {
            crate::unix_dir_fd::make_dir(&self.dir, name)?;
            let file = crate::unix_dir_fd::open_dir(Some(&self.dir), Path::new(name))
                .map_err(|error| refused_open(&path, error))?;
            Ok(PrivateDir {
                dir: crate::unix_dir_fd::checked(file, &path)?,
                path,
            })
        }
        #[cfg(not(unix))]
        open_private_dir(&path)
    }

    /// Writes `name` in this folder through a temporary file with an unpredictable name, created new, never through a
    /// link, and the owner's alone from the moment it exists (`0600` on Unix; an owner-only descriptor given to
    /// `CreateFileW` on Windows). It is then renamed over `name`; on any failure it is deleted.
    pub fn write_file(&self, name: &str, contents: &[u8]) -> io::Result<()> {
        check_entry_name(name)?;
        let temp = format!("{TEMP_PREFIX}{}.tmp", random_hex()?);
        #[cfg(unix)]
        {
            let written = crate::unix_dir_fd::write_new(&self.dir, &temp, contents)
                .and_then(|()| crate::unix_dir_fd::rename(&self.dir, &temp, name));
            if written.is_err() {
                let _ = crate::unix_dir_fd::unlink(&self.dir, &temp);
            }
            written
        }
        #[cfg(not(unix))]
        {
            let temp = self.path.join(temp);
            let written =
                write_new(&temp, contents).and_then(|()| fs::rename(&temp, self.path.join(name)));
            if written.is_err() {
                let _ = fs::remove_file(&temp);
            }
            written
        }
    }

    /// Removes the folder `name` and what it holds. It is opened without following links, so a link there is refused,
    /// then renamed through that open handle to a unique tombstone in this folder, and only the tombstone is deleted.
    /// A tombstone a crash left behind is deleted by the next `sweep_leftovers`.
    pub fn remove_child_dir(&self, name: &str) -> Result<(), RootAccessError> {
        check_entry_name(name)?;
        let tombstone = format!("{TOMBSTONE_PREFIX}{name}-{}", random_hex()?);
        let path = self.path.join(name);
        #[cfg(windows)]
        {
            let pin = crate::windows_dir_access::PinnedDir::open(&path)
                .map_err(|error| refused_open(&path, error))?;
            pin.rename_to(&self.path.join(&tombstone))?;
        }
        #[cfg(unix)]
        {
            drop(
                crate::unix_dir_fd::open_dir(Some(&self.dir), Path::new(name))
                    .map_err(|error| refused_open(&path, error))?,
            );
            crate::unix_dir_fd::rename(&self.dir, name, &tombstone)?;
        }
        delete_with_retries(&self.path.join(tombstone))?;
        Ok(())
    }

    /// Deletes the tombstones and temporary files an interrupted write or removal left in this folder. Each is tried a
    /// few times; one that still cannot be deleted is left for the next sweep.
    pub fn sweep_leftovers(&self) -> io::Result<()> {
        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            let name = entry.file_name();
            if name.to_str().is_some_and(is_leftover) {
                let _ = delete_with_retries(&entry.path());
            }
        }
        Ok(())
    }
}

/// Deletes a file, a link or a folder tree at `path`, without following links, trying a few times.
fn delete_with_retries(path: &Path) -> io::Result<()> {
    let mut attempt = 0;
    loop {
        let deleted = match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
            Ok(_) => fs::remove_file(path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        };
        attempt += 1;
        match deleted {
            Err(_) if attempt < DELETE_ATTEMPTS => std::thread::sleep(Duration::from_millis(50)),
            other => return other,
        }
    }
}

/// A new file at `path` that only the owner can open, created in one call; fails when anything is there, a link
/// included.
#[cfg(windows)]
fn write_new(path: &Path, contents: &[u8]) -> io::Result<()> {
    use std::io::Write;

    let mut file = crate::windows_profile_root::create_new_owner_only(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

#[cfg(not(any(unix, windows)))]
fn write_new(path: &Path, contents: &[u8]) -> io::Result<()> {
    use std::io::Write;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

/// 16 random bytes as hex, so a temporary or tombstone name cannot be guessed in advance.
fn random_hex() -> io::Result<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| io::Error::other("no random source"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// One folder entry: not empty, not `.` or `..`, and without a separator, a drive or stream colon, or a NUL.
pub(crate) fn check_entry_name(name: &str) -> io::Result<()> {
    let valid =
        !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', ':', '\0']);
    if valid {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name:?} is not a single folder entry"),
        ))
    }
}

/// How a no-follow open failed: a link or a file is an unsafe location, a folder another user owns is foreign.
#[cfg(any(unix, windows))]
fn refused_open(dir: &Path, error: io::Error) -> RootAccessError {
    match error.kind() {
        io::ErrorKind::InvalidInput => RootAccessError::UnsafeLocation {
            path: dir.to_path_buf(),
        },
        io::ErrorKind::PermissionDenied => RootAccessError::ForeignEntries {
            paths: vec![dir.to_path_buf()],
        },
        _ => RootAccessError::Io(error),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{TOMBSTONE_PREFIX, open_private_dir};
    use crate::root_access::RootAccessError;

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .expect("listed")
            .map(|entry| {
                entry
                    .expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    /// Grants everyone `rights` on `dir`: `M` to change it, `R` to read it.
    #[cfg(windows)]
    fn grant_everyone(dir: &Path, rights: &str) {
        let granted = std::process::Command::new("icacls")
            .arg(dir)
            .args(["/grant", &format!("*S-1-1-0:(OI)(CI)({rights})")])
            .output()
            .expect("icacls runs");
        assert!(
            granted.status.success(),
            "{}",
            String::from_utf8_lossy(&granted.stderr)
        );
    }

    #[cfg(unix)]
    fn chmod(dir: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(mode)).expect("chmod");
    }

    #[test]
    fn a_child_folder_and_its_files_are_written_through_the_open_folder_and_leave_no_temporary_file()
     {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let parent = open_private_dir(&temp.path().join("hosts")).expect("opened");
        let child = parent.child("office").expect("a child folder");
        child.write_file("host.json", b"{}").expect("written");
        child
            .write_file("host.json", b"{\"v\":2}")
            .expect("replaced");

        assert_eq!(
            fs::read(child.path().join("host.json")).expect("read"),
            b"{\"v\":2}"
        );
        assert_eq!(names(child.path()), vec!["host.json".to_owned()]);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(child.path().join("host.json"))
                .expect("metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        #[cfg(windows)]
        assert!(
            crate::windows_dir_access::has_owner_only_dacl(&child.path().join("host.json"))
                .expect("its DACL")
        );
    }

    #[test]
    fn a_file_planted_at_the_old_temporary_name_gets_no_key_bytes_and_does_not_stop_the_write() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let parent = open_private_dir(&temp.path().join("hosts")).expect("opened");
        let planted = parent.path().join("device.key.tmp");
        fs::write(&planted, b"planted").expect("a planted file");

        parent
            .write_file("device.key", b"secret key bytes")
            .expect("written");

        assert_eq!(fs::read(&planted).expect("read"), b"planted");
        assert_eq!(
            fs::read(parent.path().join("device.key")).expect("read"),
            b"secret key bytes"
        );
    }

    #[test]
    fn a_failed_write_leaves_no_temporary_file() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let parent = open_private_dir(&temp.path().join("hosts")).expect("opened");
        let blocker = parent
            .child("device.key")
            .expect("a folder where the file should go");
        blocker.write_file("inside", b"x").expect("not empty");

        assert!(parent.write_file("device.key", b"secret").is_err());

        assert_eq!(names(parent.path()), vec!["device.key".to_owned()]);
    }

    #[test]
    fn removing_a_child_folder_moves_it_aside_then_deletes_it() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let parent = open_private_dir(&temp.path().join("hosts")).expect("opened");
        parent
            .child("office")
            .expect("a child")
            .write_file("host.json", b"{}")
            .expect("a file");

        parent.remove_child_dir("office").expect("removed");

        assert!(names(parent.path()).is_empty());
    }

    #[test]
    fn a_sweep_deletes_tombstones_and_temporary_files_a_crash_left_and_keeps_the_rest() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let parent = open_private_dir(&temp.path().join("hosts")).expect("opened");
        let tombstone = parent.path().join(format!("{TOMBSTONE_PREFIX}office-dead"));
        fs::create_dir(&tombstone).expect("a tombstone");
        fs::write(tombstone.join("device.key"), b"old key").expect("a key left behind");
        fs::write(parent.path().join(".writing-0123.tmp"), b"half a key")
            .expect("a stale temporary file");
        parent.child("home").expect("a real host folder");

        parent.sweep_leftovers().expect("swept");

        assert_eq!(names(parent.path()), vec!["home".to_owned()]);
    }

    #[test]
    fn entry_names_that_are_not_one_plain_entry_are_rejected() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let parent = open_private_dir(&temp.path().join("hosts")).expect("opened");
        for name in ["", ".", "..", "a/b", "a\\b", "c:", "a\u{0}b"] {
            assert!(parent.child(name).is_err(), "{name:?}");
            assert!(parent.write_file(name, b"x").is_err(), "{name:?}");
            assert!(parent.remove_child_dir(name).is_err(), "{name:?}");
        }
    }

    #[test]
    fn an_existing_folder_others_can_write_is_refused_not_repaired() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let hosts = temp.path().join("hosts");
        drop(open_private_dir(&hosts).expect("created"));
        #[cfg(windows)]
        grant_everyone(&hosts, "M");
        #[cfg(unix)]
        chmod(&hosts, 0o777);

        assert!(matches!(
            open_private_dir(&hosts),
            Err(RootAccessError::WritableByOthers { paths }) if paths == vec![hosts.clone()]
        ));
    }

    #[cfg(windows)]
    #[test]
    fn an_existing_folder_others_can_read_is_refused() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let hosts = temp.path().join("hosts");
        drop(open_private_dir(&hosts).expect("created"));
        grant_everyone(&hosts, "R");

        assert!(matches!(
            open_private_dir(&hosts),
            Err(RootAccessError::ReadableByOthers { paths }) if paths == vec![hosts.clone()]
        ));
    }

    #[cfg(windows)]
    #[test]
    fn an_existing_folder_only_the_owners_can_reach_is_made_owner_only() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let private = temp.path().join("private");
        drop(open_private_dir(&private).expect("a private parent"));
        let hosts = private.join("hosts");
        fs::create_dir(&hosts).expect("a folder that inherits from its private parent");
        assert!(!crate::windows_dir_access::has_owner_only_dacl(&hosts).expect("its DACL"));

        drop(open_private_dir(&hosts).expect("accepted"));

        assert!(crate::windows_dir_access::has_owner_only_dacl(&hosts).expect("its DACL"));
    }

    #[cfg(unix)]
    #[test]
    fn an_existing_folder_others_can_only_read_is_tightened_to_0700() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("a temporary folder");
        let hosts = temp.path().join("hosts");
        drop(open_private_dir(&hosts).expect("created"));
        for mode in [0o755, 0o750] {
            chmod(&hosts, mode);
            drop(open_private_dir(&hosts).expect("accepted"));
            let now = fs::metadata(&hosts).expect("metadata").permissions().mode();
            assert_eq!(now & 0o777, 0o700, "{mode:o}");
        }
    }
}
