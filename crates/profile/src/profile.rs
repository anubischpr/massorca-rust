use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::endpoint::fnv1a_64;
use crate::error::ProfileError;

const DEFAULT_PROFILE: &str = "default";
const PROFILE_ENV: &str = "MASSORCA_PROFILE";
const HOME_ENV: &str = "MASSORCA_HOME";
const MAX_NAME_LEN: usize = 32;

/// One isolated profile: its directory and the names derived from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    name: String,
    home: PathBuf,
    dir: PathBuf,
}

impl Profile {
    /// Resolves from the arguments, then `MASSORCA_HOME` and `MASSORCA_PROFILE`, then the defaults.
    pub fn resolve(home: Option<PathBuf>, name: Option<String>) -> Result<Self, ProfileError> {
        Self::resolve_with(home, name, |key| std::env::var(key).ok())
    }

    /// `resolve` with the environment passed in, so tests never mutate it.
    pub fn resolve_with(
        home: Option<PathBuf>,
        name: Option<String>,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, ProfileError> {
        let name = name
            .or_else(|| non_empty(env(PROFILE_ENV)))
            .unwrap_or_else(|| DEFAULT_PROFILE.to_owned());
        if !is_valid_name(&name) {
            return Err(ProfileError::InvalidName { name });
        }
        let home = match home.or_else(|| non_empty(env(HOME_ENV)).map(PathBuf::from)) {
            Some(home) => home,
            None => default_home()?,
        };
        let home = std::path::absolute(&home).map_err(|source| ProfileError::InvalidHome {
            path: home.clone(),
            source,
        })?;
        let dir = home.join("profiles").join(&name);
        Ok(Self { name, home, dir })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn lock_path(&self) -> PathBuf {
        self.dir.join("service.lock")
    }

    pub fn info_path(&self) -> PathBuf {
        self.dir.join("service.json")
    }

    pub fn store_path(&self) -> PathBuf {
        self.dir.join("store.sqlite3")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.dir.join("logs")
    }

    /// Reserved for extensions (packages, dev links, data, lock file); `ensure_dir` does not create it.
    pub fn extensions_dir(&self) -> PathBuf {
        self.dir.join("extensions")
    }

    pub fn socket_path(&self) -> PathBuf {
        self.dir.join("service.sock")
    }

    /// Windows pipe name. FNV-1a keeps it identical across builds, which std's hashers do not. It hashes `pipe_key`,
    /// so every spelling of one directory names one pipe.
    pub fn pipe_name(&self) -> String {
        let hash = fnv1a_64(pipe_key(&self.dir).as_bytes());
        format!("massorca-{}-{hash:016x}", self.name)
    }
}

/// Creates the profile and logs directories. Extension directories are left to the extension host.
/// On Unix the profile directory is made private to its owner (0700) even when it already existed, so no other user can reach the socket in it.
pub fn ensure_dir(profile: &Profile) -> io::Result<()> {
    fs::create_dir_all(profile.dir())?;
    restrict_dir(profile.dir())?;
    fs::create_dir_all(profile.logs_dir())
}

#[cfg(unix)]
fn restrict_dir(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
}

/// Windows directories take their access from the profile's parent; the pipe carries its own descriptor.
#[cfg(not(unix))]
fn restrict_dir(_dir: &Path) -> io::Result<()> {
    Ok(())
}

/// What the pipe name hashes: the directory in one form, so every spelling of a Windows directory names one pipe.
/// NTFS ignores case and takes `/` as `\`, and `\\?\` prefixes are dropped from drive and UNC paths.
#[cfg(windows)]
fn pipe_key(dir: &Path) -> String {
    let text = dir.to_string_lossy();
    let plain = if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else if let Some(drive) = text
        .strip_prefix(r"\\?\")
        .filter(|rest| starts_with_drive_root(rest))
    {
        drive.to_owned()
    } else {
        text.into_owned()
    };
    plain.replace('/', r"\").to_lowercase()
}

/// Other platforms hash the path as given, since their case and separators are significant.
#[cfg(not(windows))]
fn pipe_key(dir: &Path) -> String {
    dir.to_string_lossy().into_owned()
}

/// Whether `text` begins with a drive root such as `C:\`.
#[cfg(windows)]
fn starts_with_drive_root(text: &str) -> bool {
    matches!(text.as_bytes(), [letter, b':', b'\\', ..] if letter.is_ascii_alphabetic())
}

/// The data directory ADR 0004 names: `%APPDATA%\MassOrca`, `$XDG_DATA_HOME/massorca`, `~/Library/Application Support/MassOrca`.
/// `ProjectDirs::data_dir()` would add a `data` folder on Windows and keep the casing on Linux.
fn default_home() -> Result<PathBuf, ProfileError> {
    let base = directories::BaseDirs::new().ok_or(ProfileError::NoHomeDirectory)?;
    let app_dir = if cfg!(target_os = "linux") {
        "massorca"
    } else {
        "MassOrca"
    };
    Ok(base.data_dir().join(app_dir))
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

fn is_valid_name(name: &str) -> bool {
    (1..=MAX_NAME_LEN).contains(&name.len())
        && name.bytes().enumerate().all(|(index, byte)| {
            let alphanumeric = byte.is_ascii_lowercase() || byte.is_ascii_digit();
            match index {
                0 => alphanumeric,
                _ => alphanumeric || byte == b'-' || byte == b'_',
            }
        })
}
