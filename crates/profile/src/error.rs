use std::io;
use std::path::PathBuf;

/// Why a profile could not be resolved.
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error(
        "invalid profile name {name:?}: use 1 to 32 characters from a-z, 0-9, '-' and '_', starting with a letter or digit"
    )]
    InvalidName { name: String },
    #[error("no data directory for this user; set MASSORCA_HOME")]
    NoHomeDirectory,
    #[error("invalid home directory {path:?}: {source}")]
    InvalidHome { path: PathBuf, source: io::Error },
}
