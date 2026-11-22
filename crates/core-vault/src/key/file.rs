//! Owner-only files of the vault directory: written once with `create_new`, read without following links.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

use zeroize::Zeroizing;

use super::{KeyError, LockReason};

/// Vault files are a key, a wrapped key or one word; anything longer is not ours.
const MAX_FILE_BYTES: usize = 4096;

/// Creates the vault directory; on Unix it is made and kept 0700, on Windows it gets an owner-only, protected DACL.
pub(super) fn ensure_private_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        if fs::symlink_metadata(dir)?.file_type().is_symlink() {
            return Err(io::Error::other("the vault directory is a symbolic link"));
        }
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
    }
    #[cfg(windows)]
    {
        super::owner_only::create_dir(dir)
    }
    #[cfg(not(any(unix, windows)))]
    fs::create_dir_all(dir)
}

/// Whether the vault directory exists. A link, a junction or anything but a directory at its path is refused, so
/// nothing is read from or written through it.
pub(super) fn check_dir(dir: &Path) -> Result<bool, KeyError> {
    let meta = match fs::symlink_metadata(dir) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if meta.file_type().is_symlink() || is_reparse_point(&meta) || !meta.is_dir() {
        return Err(KeyError::Locked(LockReason::UnsafeDirectory));
    }
    Ok(true)
}

#[cfg(windows)]
fn is_reparse_point(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
        != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_meta: &fs::Metadata) -> bool {
    false
}

/// Refuses a vault path that is a link, and on Windows gives an existing vault directory and `files` in it an
/// owner-only DACL. Returns whether the directory exists.
pub(super) fn secure_existing(dir: &Path, files: &[&str]) -> Result<bool, KeyError> {
    let exists = check_dir(dir)?;
    #[cfg(windows)]
    if exists {
        super::owner_only::tighten(dir, files)?;
    }
    #[cfg(not(windows))]
    let _ = files;
    Ok(exists)
}

pub(super) fn exists_no_follow(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Writes a new file, mode 0600 on Unix and an owner-only DACL on Windows; an existing file or link is never
/// overwritten.
pub(super) fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = create_new(path)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    if written.is_err() {
        drop(file);
        // This call created the file, so removing the partial write loses nothing.
        let _ = fs::remove_file(path);
    }
    written
}

#[cfg(windows)]
fn create_new(path: &Path) -> io::Result<File> {
    super::owner_only::create_new_file(path)
}

#[cfg(not(windows))]
fn create_new(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    options.open(path)
}

/// Reads a small vault file into a zeroizing buffer that never reallocates.
pub(super) fn read_small(path: &Path) -> io::Result<Zeroizing<Vec<u8>>> {
    let file = open_no_follow(path)?;
    let mut bytes = Zeroizing::new(Vec::with_capacity(MAX_FILE_BYTES + 1));
    let limit = u64::try_from(MAX_FILE_BYTES).unwrap_or(u64::MAX);
    file.take(limit).read_to_end(&mut bytes)?;
    if bytes.len() >= MAX_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the vault file is too large",
        ));
    }
    Ok(bytes)
}

fn open_no_follow(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(not(unix))]
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(io::Error::other("the vault file is a symbolic link"));
    }
    options.open(path)
}
