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

/// Opens the file `name` in `parent` for reading and writing, creating it with mode `0600` when it is missing, never
/// through a link (`O_NOFOLLOW`).
pub(crate) fn open_or_create(parent: &File, name: &str) -> io::Result<File> {
    let c_name = c_name(Path::new(name))?;
    // SAFETY: `parent` is an open folder and `c_name` a NUL-terminated name, both alive through the call; the mode is
    // the variadic third argument, and -1 is checked before the descriptor is owned by a File.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            c_name.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600 as libc::c_uint,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `fd` was just opened and nothing else owns it.
    Ok(unsafe { File::from_raw_fd(fd) })
}

/// The device and inode of the entry `name` of `parent`, not following a link.
pub(crate) fn entry_identity(parent: &File, name: &str) -> io::Result<(u64, u64)> {
    let c_name = c_name(Path::new(name))?;
    // SAFETY: an all-zero `stat` is a valid value to be overwritten by fstatat.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: `parent` is an open folder, `c_name` is NUL-terminated, and `stat` is a valid out-pointer.
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            c_name.as_ptr(),
            &mut stat,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok((stat.st_dev as u64, stat.st_ino as u64))
}

/// Renames the entry `from` of `parent` to `to` and checks that what moved is `expected`, the folder held open as
/// `expected`; anything else is renamed back and refused.
pub(crate) fn rename_checked(
    parent: &File,
    from: &str,
    to: &str,
    expected: &File,
) -> io::Result<()> {
    let wanted = expected.metadata()?;
    rename(parent, from, to)?;
    if entry_identity(parent, to)? == (wanted.dev(), wanted.ino()) {
        return Ok(());
    }
    let _ = rename(parent, to, from);
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("{from} changed while it was being removed"),
    ))
}

/// Whether `parent` holds an entry `name`, a link included; only `ENOENT` means absent, and any other error is
/// returned.
pub(crate) fn entry_present(parent: &File, name: &str) -> io::Result<bool> {
    let c_name = c_name(Path::new(name))?;
    // SAFETY: an all-zero `stat` is a valid value to be overwritten by fstatat.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: `parent` is an open folder, `c_name` is NUL-terminated, and `stat` is a valid out-pointer.
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            c_name.as_ptr(),
            &mut stat,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } == 0
    {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ENOENT) {
        Ok(false)
    } else {
        Err(error)
    }
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

/// The names in the folder `dir`, read through a duplicate of its descriptor; `.` and `..` are left out.
pub(crate) fn list_names(dir: &File) -> io::Result<Vec<std::ffi::OsString>> {
    use std::ffi::{CStr, OsStr};

    // SAFETY: `dir` is an open folder; dup returns a new descriptor or -1, which is checked.
    let fd = unsafe { libc::dup(dir.as_raw_fd()) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `fd` is a fresh descriptor for a folder; fdopendir takes ownership of it, or fails and leaves it open.
    let stream = unsafe { libc::fdopendir(fd) };
    if stream.is_null() {
        let error = io::Error::last_os_error();
        // SAFETY: fdopendir failed, so `fd` is still ours to close, once.
        unsafe { libc::close(fd) };
        return Err(error);
    }
    // The duplicate shares the original's position, so start from the first entry.
    // SAFETY: `stream` is an open directory stream.
    unsafe { libc::rewinddir(stream) };
    let mut names = Vec::new();
    let mut failure = None;
    loop {
        // readdir returns null both at the end and on an error; only errno tells them apart, so it is cleared first.
        set_errno(0);
        // SAFETY: `stream` is an open directory stream until closedir below.
        let entry = unsafe { libc::readdir(stream) };
        if entry.is_null() {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(0) {
                failure = Some(error);
            }
            break;
        }
        // SAFETY: `entry` points at a dirent whose name is NUL-terminated and valid until the next readdir.
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        let name = OsStr::from_bytes(name.to_bytes());
        if name != "." && name != ".." {
            names.push(name.to_os_string());
        }
    }
    // SAFETY: `stream` came from fdopendir and is closed only here, which also closes `fd`.
    unsafe { libc::closedir(stream) };
    match failure {
        Some(error) => Err(error),
        None => Ok(names),
    }
}

/// How many levels below the entry it is given `remove_entry` goes: an entry 64 levels down is removed, one 65 levels
/// down makes it refuse.
pub(crate) const MAX_REMOVE_DEPTH: usize = 64;

/// Removes the entry `name` of `parent` and, for a folder, everything in it, never following a link: a link is
/// removed itself, and each folder is opened with `O_NOFOLLOW | O_DIRECTORY` relative to its parent. The depth is
/// measured first, so a tree nested deeper than `MAX_REMOVE_DEPTH` is refused before anything is deleted.
pub(crate) fn remove_entry(parent: &File, name: &std::ffi::OsStr) -> io::Result<()> {
    check_depth(parent, name, 0)?;
    remove_entry_at(parent, name)
}

/// Fails when the entry `name` of `parent` holds anything more than `MAX_REMOVE_DEPTH` levels below it; deletes nothing.
pub(crate) fn check_depth(parent: &File, name: &std::ffi::OsStr, depth: usize) -> io::Result<()> {
    if depth > MAX_REMOVE_DEPTH {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "a folder is nested deeper than {MAX_REMOVE_DEPTH} levels, so nothing was removed"
            ),
        ));
    }
    let Some(folder) = open_child_folder(parent, name)? else {
        return Ok(());
    };
    for child in list_names(&folder)? {
        check_depth(&folder, &child, depth + 1)?;
    }
    Ok(())
}

/// The entry `name` of `parent` opened as a folder, never following a link; `None` when it is anything else.
fn open_child_folder(parent: &File, name: &std::ffi::OsStr) -> io::Result<Option<File>> {
    let c_name = CString::new(name.as_bytes())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    // SAFETY: an all-zero `stat` is a valid value to be overwritten by fstatat.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: `parent` is an open folder, `c_name` is NUL-terminated, and `stat` is a valid out-pointer.
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            c_name.as_ptr(),
            &mut stat,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    if stat.st_mode & libc::S_IFMT != libc::S_IFDIR {
        return Ok(None);
    }
    open_dir(Some(parent), Path::new(name)).map(Some)
}

fn remove_entry_at(parent: &File, name: &std::ffi::OsStr) -> io::Result<()> {
    let c_name = CString::new(name.as_bytes())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    // SAFETY: an all-zero `stat` is a valid value to be overwritten by fstatat.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: `parent` is an open folder, `c_name` is NUL-terminated, and `stat` is a valid out-pointer.
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            c_name.as_ptr(),
            &mut stat,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    let is_dir = stat.st_mode & libc::S_IFMT == libc::S_IFDIR;
    if is_dir {
        // SAFETY: as above; a returned descriptor is owned by the File made from it, and -1 is checked first.
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                c_name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `fd` was just opened and nothing else owns it.
        let folder = unsafe { File::from_raw_fd(fd) };
        for child in list_names(&folder)? {
            remove_entry_at(&folder, &child)?;
        }
    }
    let flags = if is_dir { libc::AT_REMOVEDIR } else { 0 };
    // SAFETY: `parent` is an open folder and `c_name` a NUL-terminated name, both alive through the call.
    if unsafe { libc::unlinkat(parent.as_raw_fd(), c_name.as_ptr(), flags) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Sets the calling thread's `errno`.
fn set_errno(value: libc::c_int) {
    // SAFETY: the errno location is a valid, thread-local int for the calling thread.
    unsafe { *errno_location() = value };
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "emscripten"))]
fn errno_location() -> *mut libc::c_int {
    // SAFETY: __errno_location takes no arguments and returns the calling thread's errno address.
    unsafe { libc::__errno_location() }
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "dragonfly"
))]
fn errno_location() -> *mut libc::c_int {
    // SAFETY: __error takes no arguments and returns the calling thread's errno address.
    unsafe { libc::__error() }
}

#[cfg(any(target_os = "openbsd", target_os = "netbsd"))]
fn errno_location() -> *mut libc::c_int {
    // SAFETY: __errno takes no arguments and returns the calling thread's errno address.
    unsafe { libc::__errno() }
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

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::fs::{self, File};
    use std::os::unix::fs::symlink;

    use super::{list_names, remove_entry};

    #[test]
    fn a_listing_names_a_symlink_without_following_it() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).expect("an outside folder");
        fs::write(outside.join("secret"), b"x").expect("a file outside");
        let inside = temp.path().join("inside");
        fs::create_dir(&inside).expect("a folder");
        symlink(&outside, inside.join("link")).expect("a link");

        let names = list_names(&File::open(&inside).expect("opened")).expect("listed");

        assert_eq!(names, vec![OsStr::new("link").to_os_string()]);
    }

    #[test]
    fn removing_a_symlink_or_a_folder_holding_one_never_touches_the_target() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).expect("an outside folder");
        fs::write(outside.join("secret"), b"x").expect("a file outside");
        let inside = temp.path().join("inside");
        fs::create_dir(&inside).expect("a folder");
        symlink(&outside, inside.join("link")).expect("a link");
        fs::create_dir(inside.join("tombstone")).expect("a folder to delete");
        symlink(&outside, inside.join("tombstone").join("link")).expect("a link inside it");
        let parent = File::open(&inside).expect("opened");

        remove_entry(&parent, OsStr::new("link")).expect("the link removed");
        remove_entry(&parent, OsStr::new("tombstone")).expect("the folder removed");

        assert!(fs::read_dir(&inside).expect("listed").next().is_none());
        assert_eq!(fs::read(outside.join("secret")).expect("still there"), b"x");
    }

    /// A folder `deep` with `levels` nested folders below it and a file in the last one.
    fn nested(root: &std::path::Path, levels: usize) -> std::path::PathBuf {
        let mut deepest = root.join("deep");
        for _ in 0..levels {
            deepest.push("d");
        }
        fs::create_dir_all(&deepest).expect("a deep tree");
        fs::write(deepest.join("file"), b"x").expect("a file at the bottom");
        deepest
    }

    #[test]
    fn a_tree_64_levels_deep_is_removed() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        nested(temp.path(), 63);
        let parent = File::open(temp.path()).expect("opened");

        remove_entry(&parent, OsStr::new("deep")).expect("removed");

        assert!(!temp.path().join("deep").exists());
    }

    #[test]
    fn a_tree_deeper_than_64_levels_is_refused_and_nothing_is_deleted() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let deepest = nested(temp.path(), 64);
        fs::write(temp.path().join("deep").join("near"), b"x").expect("a file near the top");
        let parent = File::open(temp.path()).expect("opened");

        assert!(remove_entry(&parent, OsStr::new("deep")).is_err());

        assert!(deepest.join("file").is_file(), "the deepest file stays");
        assert!(
            temp.path().join("deep").join("near").is_file(),
            "nothing was deleted"
        );
    }
}
