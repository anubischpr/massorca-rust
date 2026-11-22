//! A folder of the profile held open without following links, and the entries it creates, writes and removes
//! (ADR 0073). On Windows the folder is pinned, so it cannot be renamed or replaced while it is held, and its entries are
//! reached through its path. On Unix entries are created, opened, renamed and deleted relative to the folder's
//! descriptor (`openat`, `mkdirat`, `renameat`, `unlinkat`), never through a link. A removed folder is first renamed
//! through an open handle; on Windows the renamed folder is then deleted by path inside the held folder with
//! `fs::remove_dir_all`, which deletes links themselves and never follows them.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::root_access::{RootAccessError, platform_create_private_dir};

/// Names starting with this are folders being removed; `list`-style readers skip them and `sweep_leftovers` deletes them.
pub const TOMBSTONE_PREFIX: &str = ".removing-";

/// Names starting with this are files being written.
const TEMP_PREFIX: &str = ".writing-";

/// The file whose lock `lock` takes; readers skip it, since it is not a folder.
const LOCK_FILE: &str = ".lock";

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
    pin: crate::windows_dir_access::PinnedDir,
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
    use crate::windows_profile_root::{
        FolderAccess, entries_others_can_reach, folder_access, make_owner_only_below,
    };

    let paths = || vec![dir.to_path_buf()];
    let held = pin_waiting(dir)?;
    match folder_access(dir)? {
        FolderAccess::Private => {}
        // Entries below may have inherited a grant the folder only passed on (inherit-only), so everything below is
        // made owner-only too, through handles and while the folder stays pinned.
        FolderAccess::OwnersOnly => make_owner_only_below(dir)?,
        FolderAccess::WritableByOthers => {
            return Err(RootAccessError::WritableByOthers { paths: paths() });
        }
        FolderAccess::ReadableByOthers => {
            return Err(RootAccessError::ReadableByOthers { paths: paths() });
        }
    }
    // The folder holds secrets, so anything below it that others can reach is refused on every open.
    let exposed = entries_others_can_reach(dir)?;
    if !exposed.is_empty() {
        return Err(RootAccessError::ReadableByOthers { paths: exposed });
    }
    Ok(PrivateDir {
        path: dir.to_path_buf(),
        pin: held,
    })
}

/// `ERROR_SHARING_VIOLATION`: another handle holds the folder without sharing delete, such as another add's pin.
#[cfg(windows)]
const SHARING_VIOLATION: i32 = 32;

/// Pins `dir`, waiting up to about two seconds while another pin of it is held, so two writers take turns instead of
/// one failing.
#[cfg(windows)]
fn pin_waiting(dir: &Path) -> Result<crate::windows_dir_access::PinnedDir, RootAccessError> {
    let mut attempts = 0;
    loop {
        match crate::windows_dir_access::PinnedDir::open(dir) {
            Ok(pin) => return Ok(pin),
            Err(error) if error.raw_os_error() == Some(SHARING_VIOLATION) && attempts < 100 => {
                attempts += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => return Err(refused_open(dir, error)),
        }
    }
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

    /// Takes the exclusive lock on this folder's `.lock` file, waiting while another writer holds it, in this process or
    /// another; the lock ends when the returned file is dropped. Writers that sweep leftovers take it first, so a sweep
    /// never deletes the staged folder of a write still in progress.
    pub fn lock(&self) -> io::Result<fs::File> {
        #[cfg(unix)]
        let file = crate::unix_dir_fd::open_or_create(&self.dir, LOCK_FILE)?;
        #[cfg(windows)]
        let file = {
            use std::os::windows::fs::OpenOptionsExt;

            // The folder is pinned and private, so its lock file is opened by path, never through a link.
            const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
            fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
                .open(self.path.join(LOCK_FILE))?
        };
        #[cfg(not(any(unix, windows)))]
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.path.join(LOCK_FILE))?;
        file.lock()?;
        Ok(file)
    }

    /// A new, empty, private folder in this one under a random `.writing-` name, to fill and then `publish` under its
    /// real name in one rename. Until then readers skip it, and a sweep deletes it after a crash.
    pub fn staged_child(&self) -> Result<PrivateDir, RootAccessError> {
        self.child(&format!("{TEMP_PREFIX}{}", random_hex()?))
    }

    /// Renames `staged`, a folder `staged_child` made in this one, to `name` through its open handle. Fails when
    /// anything is at `name`.
    pub fn publish(&self, staged: PrivateDir, name: &str) -> Result<(), RootAccessError> {
        check_entry_name(name)?;
        #[cfg(windows)]
        staged.pin.rename_to(&self.path.join(name))?;
        #[cfg(unix)]
        {
            let staged_name = staged
                .path
                .file_name()
                .and_then(|staged_name| staged_name.to_str())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "a staged folder has no name")
                })?
                .to_owned();
            if crate::unix_dir_fd::list_names(&self.dir)?
                .iter()
                .any(|entry| entry == name)
            {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("{name} already exists"),
                )
                .into());
            }
            crate::unix_dir_fd::rename(&self.dir, &staged_name, name)?;
        }
        #[cfg(not(any(unix, windows)))]
        fs::rename(&staged.path, self.path.join(name))?;
        Ok(())
    }

    /// Deletes `staged`, a folder `staged_child` made in this one that will not be published.
    pub fn discard(&self, staged: PrivateDir) -> io::Result<()> {
        let name = staged
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "a staged folder has no name")
            })?
            .to_owned();
        drop(staged);
        self.delete_entry(&name)
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

    /// Removes the folder `name` like `remove_child_dir`, unless it holds an entry `marker`: that is checked while the
    /// folder itself is held open, right before it is moved aside, so a folder that gains the marker in the meantime is
    /// never removed. Returns whether it was removed. Only a definite "not found" counts as absent; any other error
    /// when reading the marker removes nothing and is returned.
    pub fn remove_child_dir_unless(
        &self,
        name: &str,
        marker: &str,
    ) -> Result<bool, RootAccessError> {
        check_entry_name(name)?;
        check_entry_name(marker)?;
        let tombstone = format!("{TOMBSTONE_PREFIX}{name}-{}", random_hex()?);
        let path = self.path.join(name);
        #[cfg(windows)]
        {
            let pin = crate::windows_dir_access::PinnedDir::open(&path)
                .map_err(|error| refused_open(&path, error))?;
            if entry_present(&path.join(marker))? {
                return Ok(false);
            }
            pin.rename_to(&self.path.join(&tombstone))?;
        }
        #[cfg(unix)]
        {
            let folder = crate::unix_dir_fd::open_dir(Some(&self.dir), Path::new(name))
                .map_err(|error| refused_open(&path, error))?;
            if crate::unix_dir_fd::entry_present(&folder, marker)? {
                return Ok(false);
            }
            crate::unix_dir_fd::rename(&self.dir, name, &tombstone)?;
        }
        self.delete_entry(&tombstone)?;
        Ok(true)
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
        self.delete_entry(&tombstone)?;
        Ok(())
    }

    /// Deletes the entry `name` of this folder and what it holds, without following links, trying a few times: on
    /// Unix relative to the folder's descriptor, on Windows by path inside the held folder.
    fn delete_entry(&self, name: &str) -> io::Result<()> {
        with_retries(|| {
            #[cfg(unix)]
            {
                match crate::unix_dir_fd::remove_entry(&self.dir, std::ffi::OsStr::new(name)) {
                    Err(error) if error.raw_os_error() == Some(libc::ENOENT) => Ok(()),
                    other => other,
                }
            }
            #[cfg(not(unix))]
            {
                delete_path(&self.path.join(name))
            }
        })
    }

    /// Deletes the tombstones and temporary files an interrupted write or removal left in this folder. Each is tried a
    /// few times; one that still cannot be deleted is left for the next sweep.
    pub fn sweep_leftovers(&self) -> io::Result<()> {
        #[cfg(unix)]
        let names = crate::unix_dir_fd::list_names(&self.dir)?;
        #[cfg(not(unix))]
        let names = fs::read_dir(&self.path)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        for name in names {
            if let Some(name) = name.to_str().filter(|name| is_leftover(name)) {
                let _ = self.delete_entry(name);
            }
        }
        Ok(())
    }
}

/// Whether `path` exists; only "not found" means absent, and any other error is returned.
#[cfg(windows)]
fn entry_present(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Runs `delete` up to `DELETE_ATTEMPTS` times, pausing briefly between failures.
fn with_retries(mut delete: impl FnMut() -> io::Result<()>) -> io::Result<()> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match delete() {
            Err(_) if attempt < DELETE_ATTEMPTS => std::thread::sleep(Duration::from_millis(50)),
            other => return other,
        }
    }
}

/// Deletes a file, a link or a folder tree at `path`, without following links.
#[cfg(not(unix))]
fn delete_path(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
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
        let granted = massorca_process::command("icacls")
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

    /// A hosts folder that passes a read grant for everyone on to what is created in it, without holding it itself.
    #[cfg(windows)]
    fn hosts_passing_on_a_read_grant(temp: &Path) -> std::path::PathBuf {
        let hosts = temp.join("hosts");
        drop(open_private_dir(&hosts).expect("created"));
        let user = crate::windows_trust::current_user().expect("the user");
        crate::windows_profile_root::set_dacl(
            &hosts,
            &format!("D:P(A;OICI;FA;;;{user})(A;OICIIO;FR;;;WD)"),
            true,
        )
        .expect("an inherit-only grant");
        hosts
    }

    #[cfg(windows)]
    fn others_can_reach(path: &Path) -> bool {
        use crate::windows_profile_root::{Links, read_sddl};
        let descriptor = crate::windows_sddl::Descriptor::parse(
            &read_sddl(path, Links::Open).expect("its DACL"),
        );
        descriptor.grants_any_to_others(&crate::windows_trust::Trusted::current().expect("trusted"))
    }

    #[cfg(windows)]
    #[test]
    fn tightening_a_folder_takes_an_inherited_read_grant_away_from_what_is_below_it() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let hosts = hosts_passing_on_a_read_grant(temp.path());
        let office = hosts.join("office");
        fs::create_dir(&office).expect("a host folder");
        fs::write(office.join("device.key"), b"key").expect("a key");
        assert!(
            others_can_reach(&office),
            "the folder inherited the read grant"
        );
        assert!(
            others_can_reach(&office.join("device.key")),
            "the key inherited it too"
        );

        drop(open_private_dir(&hosts).expect("tightened"));

        assert!(crate::windows_dir_access::has_owner_only_dacl(&hosts).expect("its DACL"));
        assert!(!others_can_reach(&office));
        assert!(!others_can_reach(&office.join("device.key")));
    }

    #[cfg(windows)]
    #[test]
    fn an_explicit_grant_below_a_tightened_folder_is_refused() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let hosts = hosts_passing_on_a_read_grant(temp.path());
        let office = hosts.join("office");
        fs::create_dir(&office).expect("a host folder");
        crate::windows_profile_root::set_dacl(&office, "D:(A;;FR;;;WD)", false)
            .expect("an explicit grant");

        assert!(matches!(
            open_private_dir(&hosts),
            Err(RootAccessError::ReadableByOthers { paths }) if paths == vec![office.clone()]
        ));
    }

    #[cfg(windows)]
    #[test]
    fn tightening_refuses_a_junction_below_and_changes_nothing_outside() {
        use crate::windows_profile_root::{Links, read_sddl};

        let temp = tempfile::tempdir().expect("a temporary folder");
        let user = crate::windows_trust::current_user().expect("the user");
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).expect("an outside folder");
        fs::write(outside.join("file.txt"), b"x").expect("a file in it");
        crate::windows_profile_root::set_dacl(
            &outside,
            &format!("D:P(A;OICI;FA;;;{user})(A;OICI;FR;;;WD)"),
            true,
        )
        .expect("a known DACL");
        let before = (
            read_sddl(&outside, Links::Open).expect("its DACL"),
            read_sddl(&outside.join("file.txt"), Links::Open).expect("its DACL"),
        );
        let hosts = hosts_passing_on_a_read_grant(temp.path());
        let link = hosts.join("office");
        let made = massorca_process::command("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .expect("mklink runs");
        assert!(
            made.status.success(),
            "{}",
            String::from_utf8_lossy(&made.stderr)
        );

        assert!(matches!(
            open_private_dir(&hosts),
            Err(RootAccessError::UnsafeLocation { path }) if path == link
        ));

        let after = (
            read_sddl(&outside, Links::Open).expect("its DACL"),
            read_sddl(&outside.join("file.txt"), Links::Open).expect("its DACL"),
        );
        assert_eq!(after, before, "nothing outside the hosts folder changed");
        assert!(others_can_reach(&outside));
    }

    #[cfg(windows)]
    #[test]
    fn an_explicit_read_grant_below_an_owner_only_folder_is_refused_on_every_open() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let hosts = temp.path().join("hosts");
        let parent = open_private_dir(&hosts).expect("an owner-only folder");
        let office = parent.child("office").expect("a host folder");
        office.write_file("device.key", b"key").expect("a key");
        drop(office);
        drop(parent);
        let key = hosts.join("office").join("device.key");
        crate::windows_profile_root::set_dacl(&key, "D:(A;;FR;;;WD)", false)
            .expect("an explicit grant");

        assert!(matches!(
            open_private_dir(&hosts),
            Err(RootAccessError::ReadableByOthers { paths }) if paths == vec![key.clone()]
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_sweep_deletes_a_tombstone_holding_a_symlink_without_touching_the_target() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).expect("an outside folder");
        fs::write(outside.join("secret"), b"x").expect("a file outside");
        let parent = open_private_dir(&temp.path().join("hosts")).expect("opened");
        let tombstone = parent.path().join(format!("{TOMBSTONE_PREFIX}office-dead"));
        fs::create_dir(&tombstone).expect("a tombstone");
        std::os::unix::fs::symlink(&outside, tombstone.join("link")).expect("a link inside it");
        std::os::unix::fs::symlink(&outside, parent.path().join(".writing-link"))
            .expect("a leftover link");

        parent.sweep_leftovers().expect("swept");

        assert!(names(parent.path()).is_empty());
        assert_eq!(fs::read(outside.join("secret")).expect("still there"), b"x");
    }
}
