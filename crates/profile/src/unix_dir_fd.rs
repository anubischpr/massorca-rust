//! Folder entries reached relative to an open folder's descriptor, never through a path that a symlink could redirect
//! (ADR 0073). Every unsafe call states, in its `SAFETY` comment, the invariant it relies on.
#![allow(unsafe_code)]

use std::ffi::CString;
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

use crate::root_access::RootAccessError;
use crate::unix_user::current_uid;

/// Opens the folder `path`, relative to `parent` when it is given, with `O_NOFOLLOW | O_DIRECTORY`. A symlink or a file
/// fails with `InvalidInput`.
pub(crate) fn open_dir(parent: Option<&File>, path: &Path) -> io::Result<File> {
    let opened = match parent {
        None => fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
            .open(path),
        Some(parent) => {
            let name = c_name(path)?;
            // SAFETY: `parent` is an open folder and `name` a NUL-terminated name, both alive through the call; a
            // returned descriptor is owned by the File made from it below, and -1 is checked first.
            let fd = unsafe {
                libc::openat(
                    parent.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                Err(io::Error::last_os_error())
            } else {
                // SAFETY: `fd` was just opened and nothing else owns it.
                Ok(unsafe { File::from_raw_fd(fd) })
            }
        }
    };
    opened.map_err(|error| match error.raw_os_error() {
        Some(libc::ELOOP | libc::ENOTDIR) => io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is a link or not a folder", path.display()),
        ),
        _ => error,
    })
}

/// The folder `file` holds, if the user owns it and others cannot write it; it is then set `0700` through the
/// descriptor. Group or world write is refused, not repaired: what the folder holds could have been changed.
pub(crate) fn checked(file: File, path: &Path) -> Result<File, RootAccessError> {
    let metadata = file.metadata()?;
    if metadata.uid() != current_uid() {
        return Err(RootAccessError::ForeignEntries {
            paths: vec![path.to_path_buf()],
        });
    }
    if metadata.mode() & 0o022 != 0 {
        return Err(RootAccessError::WritableByOthers {
            paths: vec![path.to_path_buf()],
        });
    }
    file.set_permissions(fs::Permissions::from_mode(0o700))?;
    Ok(file)
}

/// Creates the folder `name` in `parent` with mode `0700`; an existing entry is left for the no-follow open to judge.
pub(crate) fn make_dir(parent: &File, name: &str) -> io::Result<()> {
    let name = c_name(Path::new(name))?;
    // SAFETY: `parent` is an open folder and `name` a NUL-terminated name, both alive through the call.
    if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::EEXIST) {
        Ok(())
    } else {
        Err(error)
    }
}

/// Writes `contents` to a new file `name` in `parent`, mode `0600`, created with `O_EXCL | O_NOFOLLOW`: it fails when
/// anything is there, so it is never reached through a link.
pub(crate) fn write_new(parent: &File, name: &str, contents: &[u8]) -> io::Result<()> {
    let c_name = c_name(Path::new(name))?;
    // SAFETY: `parent` is an open folder and `c_name` a NUL-terminated name, both alive through the call; the mode is passed as the variadic third argument, and -1 is checked before the descriptor
    // is owned by a File.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            c_name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600 as libc::c_uint,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `fd` was just opened and nothing else owns it.
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(contents)?;
    file.sync_all()
}

/// Removes the file `name` from `parent`; a link is removed itself, never its target.
pub(crate) fn unlink(parent: &File, name: &str) -> io::Result<()> {
    let name = c_name(Path::new(name))?;
    // SAFETY: `parent` is an open folder and `name` a NUL-terminated name, both alive through the call.
    if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Renames `from` to `to`, both entries of `parent`.
pub(crate) fn rename(parent: &File, from: &str, to: &str) -> io::Result<()> {
    let from = c_name(Path::new(from))?;
    let to = c_name(Path::new(to))?;
    // SAFETY: `parent` is an open folder and both names are NUL-terminated, all alive through the call.
    if unsafe {
        libc::renameat(
            parent.as_raw_fd(),
            from.as_ptr(),
            parent.as_raw_fd(),
            to.as_ptr(),
        )
    } == 0
    {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// `name` as a C string, once `check_entry_name` accepts it as one folder entry.
fn c_name(name: &Path) -> io::Result<CString> {
    let text = name.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not UTF-8", name.display()),
        )
    })?;
    crate::private_dir::check_entry_name(text)?;
    CString::new(name.as_os_str().as_bytes())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
}
