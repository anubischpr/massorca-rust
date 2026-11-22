//! Owner-only files of the vault directory: written once with `create_new`, read without following links.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

use zeroize::Zeroizing;

/// Vault files are a key, a wrapped key or one word; anything longer is not ours.
const MAX_FILE_BYTES: usize = 4096;

/// Creates the vault directory; on Unix it is made and kept 0700.
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
    #[cfg(not(unix))]
    fs::create_dir_all(dir)
}

pub(super) fn exists_no_follow(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Writes a new file, mode 0600 on Unix; an existing file or link is never overwritten.
pub(super) fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    if written.is_err() {
        drop(file);
        // This call created the file, so removing the partial write loses nothing.
        let _ = fs::remove_file(path);
    }
    written
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
