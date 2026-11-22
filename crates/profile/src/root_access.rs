//! Access of the profile root (ADR 0073): created private to its owner, and checked before every service start.

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
    /// Only trusted users could reach the root, but its access was not the owner-only one; it is now.
    Repaired,
}

/// Why a profile root is not trusted. Nothing is repaired when the location or the root's own access is refused.
#[derive(Debug, thiserror::Error)]
pub enum RootAccessError {
    /// The root, or entries below it, belong to another user and may have been planted. Up to `MAX_LISTED_ENTRIES` paths.
    #[error(
        "the profile directory holds entries another user owns, which could have been planted: {}; remove them or take ownership, then start again",
        list_paths(paths)
    )]
    ForeignEntries { paths: Vec<PathBuf> },
    /// Other users could change the root or entries below it, so files the user owns may have been rewritten.
    #[error(
        "the profile directory was writable by other users, so its content may have been changed: {}; other programs on this computer could change these files. Review them, then run `massorca profile secure` to make the home private",
        list_paths(paths)
    )]
    WritableByOthers { paths: Vec<PathBuf> },
    /// A folder from the home down to the root is a link, belongs to another user, or other users can change it.
    #[error(
        "the profile location is unsafe: {} is a link, belongs to another user, or other users can change it; other programs on this computer could change these files. Run `massorca profile secure` to make the home private, or set MASSORCA_HOME to a folder only you can write to",
        path.display()
    )]
    UnsafeLocation { path: PathBuf },
    #[error("cannot check the profile directory: {0}")]
    Io(#[from] io::Error),
}

/// Checks the home and `profiles` folders, then creates the profile root private to its owner or checks the existing
/// one: repairs its access when only trusted users could reach it, and refuses it otherwise (ADR 0073).
pub fn secure_root(profile: &Profile) -> Result<RootAccess, RootAccessError> {
    platform_secure_root(profile.home(), profile.dir())
}

/// Creates whatever is missing of the home, `profiles` and the root, each private to its owner in one step; folders
/// above the home are created as usual. Returns whether the root was created. Existing folders are left as they are.
pub fn create_root(profile: &Profile) -> io::Result<bool> {
    create_home(profile.home())?;
    if let Some(profiles) = profile.dir().parent() {
        platform_create_private_dir(profiles)?;
    }
    platform_create_private_dir(profile.dir())
}

/// Creates `home` private to its owner in one step when it is missing, and the folders above it as usual. Returns
/// whether it was created; an existing home is left as it is.
pub fn create_home(home: &Path) -> io::Result<bool> {
    if let Some(parent) = home.parent() {
        std::fs::create_dir_all(parent)?;
    }
    platform_create_private_dir(home)
}

/// Creates `dir` private to its owner in one step, inside a folder that must exist; false when `dir` already existed,
/// which is then left as it is. For folders of the profile, such as `hosts`, that must never exist with wider access.
pub fn create_private_dir(dir: &Path) -> io::Result<bool> {
    platform_create_private_dir(dir)
}

/// Checks the location without creating anything: a home that is a link needs a safe parent, an existing home and
/// `profiles` must pass the ancestor check, and an existing root must not be a link. Run before folders are created
/// for a profile that has not been checked yet, so a link there cannot redirect the creation.
pub fn check_location(profile: &Profile) -> Result<(), RootAccessError> {
    platform_check_location(profile.home(), profile.dir())
}

#[cfg(windows)]
fn platform_check_location(home: &Path, dir: &Path) -> Result<(), RootAccessError> {
    crate::windows_profile_root::check_location(home, dir)
}

#[cfg(unix)]
fn platform_check_location(home: &Path, dir: &Path) -> Result<(), RootAccessError> {
    crate::unix_profile_root::check_location(home, dir)
}

#[cfg(not(any(unix, windows)))]
fn platform_check_location(_home: &Path, _dir: &Path) -> Result<(), RootAccessError> {
    Ok(())
}

/// What `profile secure` changes in one folder of the profile's location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderPlan {
    pub path: PathBuf,
    /// The access entries it removes; empty when the folder is already private. Windows shows DACL entries as SDDL,
    /// Unix the permission mode.
    pub remove: Vec<String>,
    /// What is left afterwards: full access for the user alone.
    pub keep: Vec<String>,
}

impl FolderPlan {
    pub fn changes(&self) -> bool {
        !self.remove.is_empty()
    }
}

/// What `profile secure` would change in the home, `profiles` and the profile root, without changing anything.
/// Refuses a location it cannot make safe: a link, or a folder another user owns.
pub fn plan_secure(profile: &Profile) -> Result<Vec<FolderPlan>, RootAccessError> {
    platform_plan_secure(profile.home(), profile.dir())
}

/// Makes the home, `profiles` and the root private to the user (owner-only, nothing inherited from above, passed on
/// to everything below), then checks the root as a start does, reading every entry. The owner asks for this
/// explicitly; a start never does it on its own.
pub fn apply_secure(profile: &Profile) -> Result<RootAccess, RootAccessError> {
    platform_apply_secure(profile.home(), profile.dir())?;
    #[cfg(windows)]
    crate::scan_cache::ScanCache::clear(profile.dir());
    secure_root(profile)
}

/// The `profiles` folder between the home and the root.
pub(crate) fn profiles_dir<'a>(home: &'a Path, root: &'a Path) -> &'a Path {
    root.parent().unwrap_or(home)
}

/// Collects up to `MAX_LISTED_ENTRIES` paths of each kind the scan below the root refuses.
#[derive(Debug, Default)]
pub(crate) struct Findings {
    pub foreign: Vec<PathBuf>,
    pub writable: Vec<PathBuf>,
}

impl Findings {
    pub fn add_foreign(&mut self, path: PathBuf) {
        if self.foreign.len() < MAX_LISTED_ENTRIES {
            self.foreign.push(path);
        }
    }

    pub fn add_writable(&mut self, path: PathBuf) {
        if self.writable.len() < MAX_LISTED_ENTRIES {
            self.writable.push(path);
        }
    }

    /// Whether more scanning cannot change the result: foreign entries are reported first.
    pub fn is_full(&self) -> bool {
        self.foreign.len() >= MAX_LISTED_ENTRIES
    }

    pub fn into_result(self) -> Result<(), RootAccessError> {
        if !self.foreign.is_empty() {
            return Err(RootAccessError::ForeignEntries {
                paths: self.foreign,
            });
        }
        if !self.writable.is_empty() {
            return Err(RootAccessError::WritableByOthers {
                paths: self.writable,
            });
        }
        Ok(())
    }
}

#[cfg(windows)]
fn platform_secure_root(home: &Path, dir: &Path) -> Result<RootAccess, RootAccessError> {
    crate::windows_profile_root::secure_root(home, dir)
}

#[cfg(windows)]
fn platform_plan_secure(home: &Path, dir: &Path) -> Result<Vec<FolderPlan>, RootAccessError> {
    crate::windows_profile_secure::plan(home, dir)
}

#[cfg(windows)]
fn platform_apply_secure(home: &Path, dir: &Path) -> Result<(), RootAccessError> {
    crate::windows_profile_secure::apply(home, dir)
}

#[cfg(windows)]
pub(crate) fn platform_create_private_dir(dir: &Path) -> io::Result<bool> {
    crate::windows_profile_root::create_private_dir(dir)
}

#[cfg(unix)]
fn platform_secure_root(home: &Path, dir: &Path) -> Result<RootAccess, RootAccessError> {
    crate::unix_profile_root::secure_root(home, dir)
}

#[cfg(unix)]
fn platform_plan_secure(home: &Path, dir: &Path) -> Result<Vec<FolderPlan>, RootAccessError> {
    crate::unix_profile_root::plan_secure(home, dir)
}

#[cfg(unix)]
fn platform_apply_secure(home: &Path, dir: &Path) -> Result<(), RootAccessError> {
    crate::unix_profile_root::apply_secure(home, dir)
}

#[cfg(unix)]
pub(crate) fn platform_create_private_dir(dir: &Path) -> io::Result<bool> {
    crate::unix_profile_root::create_private_dir(dir)
}

/// Platforms without owner checks create the folders and leave their access alone.
#[cfg(not(any(unix, windows)))]
fn platform_secure_root(home: &Path, dir: &Path) -> Result<RootAccess, RootAccessError> {
    std::fs::create_dir_all(profiles_dir(home, dir))?;
    Ok(if platform_create_private_dir(dir)? {
        RootAccess::Created
    } else {
        RootAccess::Private
    })
}

#[cfg(not(any(unix, windows)))]
fn platform_plan_secure(_home: &Path, _dir: &Path) -> Result<Vec<FolderPlan>, RootAccessError> {
    Ok(Vec::new())
}

#[cfg(not(any(unix, windows)))]
fn platform_apply_secure(_home: &Path, _dir: &Path) -> Result<(), RootAccessError> {
    Ok(())
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn platform_create_private_dir(dir: &Path) -> io::Result<bool> {
    match std::fs::create_dir(dir) {
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
