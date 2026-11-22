//! A folder of the profile held open without following links, and the entries it creates, writes and removes
//! (ADR 0073). On Windows the folder is pinned, so it cannot be renamed or replaced while it is held, and its entries are
//! reached through its path. On Unix every entry is reached relative to the folder's descriptor (`openat`, `mkdirat`,
//! `renameat`, `unlinkat`), so a link swapped in for the folder or a parent redirects nothing.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::root_access::{RootAccessError, platform_create_private_dir};

#[derive(Debug)]
pub struct PrivateDir {
    path: PathBuf,
    #[cfg(windows)]
    _pin: crate::windows_dir_access::PinnedDir,
    #[cfg(unix)]
    dir: fs::File,
}

/// Creates `dir` private to its owner when it is missing, then opens it without following links. Refuses a link or a
/// file there with `UnsafeLocation`, a folder another user owns with `ForeignEntries`, and a folder other users could
/// change with `WritableByOthers`: its content cannot be trusted, so it is refused, not repaired. Run `secure_root`
/// first, so the folder it sits in is checked.
pub fn open_private_dir(dir: &Path) -> Result<PrivateDir, RootAccessError> {
    platform_create_private_dir(dir)?;
    open_existing(dir)
}

#[cfg(windows)]
fn open_existing(dir: &Path) -> Result<PrivateDir, RootAccessError> {
    let pin = crate::windows_dir_access::PinnedDir::open(dir)
        .map_err(|error| refused_open(dir, error))?;
    if crate::windows_profile_root::writable_by_others(dir)? {
        return Err(RootAccessError::WritableByOthers {
            paths: vec![dir.to_path_buf()],
        });
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

    /// Writes `name` in this folder: a temporary file created new and never through a link, then renamed over `name`.
    /// The file is the owner's alone before any byte lands: `0600` on Unix, an owner-only DACL on Windows.
    pub fn write_file(&self, name: &str, contents: &[u8]) -> io::Result<()> {
        let temp = format!("{name}.tmp");
        #[cfg(unix)]
        {
            crate::unix_dir_fd::write_new(&self.dir, &temp, contents)?;
            crate::unix_dir_fd::rename(&self.dir, &temp, name)
        }
        #[cfg(not(unix))]
        {
            use std::io::Write;

            let temp = self.path.join(temp);
            match fs::remove_file(&temp) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
                _ => {}
            }
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            #[cfg(windows)]
            crate::windows_endpoint_access::restrict_to_owner(&temp)?;
            file.write_all(contents)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temp, self.path.join(name))
        }
    }

    /// Removes the folder `name` and what it holds. It is opened without following links, so a link there is refused,
    /// then moved aside under a unique name through that open folder, and only the moved folder is deleted.
    pub fn remove_child_dir(&self, name: &str) -> Result<(), RootAccessError> {
        let tombstone = tombstone_name(name);
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
        fs::remove_dir_all(self.path.join(tombstone))?;
        Ok(())
    }
}

/// A name nothing else uses: the folder's name, this process and the time.
fn tombstone_name(name: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    format!(".removing-{name}-{}-{nanos}", std::process::id())
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

    use super::open_private_dir;
    use crate::root_access::RootAccessError;

    #[test]
    fn a_child_folder_and_its_files_are_written_through_the_open_folder() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let parent = open_private_dir(&temp.path().join("hosts")).expect("opened");
        let child = parent.child("office").expect("a child folder");
        child.write_file("host.json", b"{}").expect("written");
        child
            .write_file("host.json", b"{\"v\":2}")
            .expect("replaced");

        assert_eq!(
            fs::read(temp.path().join("hosts").join("office").join("host.json")).expect("read"),
            b"{\"v\":2}"
        );
        assert!(
            !temp
                .path()
                .join("hosts")
                .join("office")
                .join("host.json.tmp")
                .exists()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(child.path().join("host.json"))
                .expect("metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
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

        let left: Vec<_> = fs::read_dir(parent.path()).expect("listed").collect();
        assert!(left.is_empty(), "{left:?}");
    }

    #[test]
    fn an_existing_folder_others_can_write_is_refused_not_repaired() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let hosts = temp.path().join("hosts");
        drop(open_private_dir(&hosts).expect("created"));
        #[cfg(windows)]
        {
            let user = crate::windows_trust::current_user().expect("the user");
            crate::windows_profile_root::set_dacl(
                &hosts,
                &format!("D:P(A;OICI;FA;;;{user})(A;OICI;FA;;;WD)"),
                true,
            )
            .expect("a grant to everyone");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hosts, fs::Permissions::from_mode(0o777)).expect("chmod");
        }

        assert!(matches!(
            open_private_dir(&hosts),
            Err(RootAccessError::WritableByOthers { paths }) if paths == vec![hosts.clone()]
        ));
    }
}
