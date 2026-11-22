//! Access of the profile root (ADR 0073): created private to its owner, and checked before every service start.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::profile::Profile;

/// At most this many paths are named when a profile root is refused.
pub const MAX_LISTED_ENTRIES: usize = 10;

/// What `secure_root` found and did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootAccess {
    /// The root did not exist and was created private.
    Created,
    /// The root was already private.
    Private,
    /// Others could reach the root; it is private now.
    Repaired,
}

/// Why a profile root is not trusted.
#[derive(Debug, thiserror::Error)]
pub enum RootAccessError {
    /// The root, or entries below it, belong to another user and may have been planted. Up to `MAX_LISTED_ENTRIES` paths.
    #[error(
        "the profile directory holds entries another user owns, which could have been planted: {}; remove them or take ownership, then start again",
        list_paths(paths)
    )]
    ForeignEntries { paths: Vec<PathBuf> },
    #[error("cannot check the profile directory: {0}")]
    Io(#[from] io::Error),
}

/// Creates the profile root private to its owner, or checks an existing one: repairs its access when the current user
/// owns it, and refuses it when it or an entry below it belongs to another user.
pub fn secure_root(profile: &Profile) -> Result<RootAccess, RootAccessError> {
    platform_secure_root(profile.dir())
}

/// Creates `dir` private to its owner in one step; false when it already existed. Missing parents are created as usual.
pub(crate) fn create_private_dir(dir: &Path) -> io::Result<bool> {
    if let Some(parent) = dir.parent() {
        fs::create_dir_all(parent)?;
    }
    platform_create_private_dir(dir)
}

#[cfg(windows)]
fn platform_secure_root(dir: &Path) -> Result<RootAccess, RootAccessError> {
    crate::windows_profile_root::secure_root(dir)
}

#[cfg(windows)]
fn platform_create_private_dir(dir: &Path) -> io::Result<bool> {
    crate::windows_profile_root::create_private_dir(dir)
}

#[cfg(unix)]
fn platform_secure_root(dir: &Path) -> Result<RootAccess, RootAccessError> {
    use std::os::unix::fs::PermissionsExt;

    if create_private_dir(dir)? {
        return Ok(RootAccess::Created);
    }
    let mode = fs::metadata(dir)?.permissions().mode() & 0o777;
    if mode == 0o700 {
        return Ok(RootAccess::Private);
    }
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    Ok(RootAccess::Repaired)
}

#[cfg(unix)]
fn platform_create_private_dir(dir: &Path) -> io::Result<bool> {
    use std::os::unix::fs::DirBuilderExt;

    match fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error),
    }
}

/// Platforms without owner checks create the directory and leave its access alone.
#[cfg(not(any(unix, windows)))]
fn platform_secure_root(dir: &Path) -> Result<RootAccess, RootAccessError> {
    Ok(if create_private_dir(dir)? {
        RootAccess::Created
    } else {
        RootAccess::Private
    })
}

#[cfg(not(any(unix, windows)))]
fn platform_create_private_dir(dir: &Path) -> io::Result<bool> {
    match fs::create_dir(dir) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error),
    }
}

fn list_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}
