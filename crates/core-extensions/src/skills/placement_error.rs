//! Everything a skill placement refuses or fails at.

use std::io;
use std::path::PathBuf;

/// Everything a placement refuses or fails at.
#[derive(Debug, thiserror::Error)]
pub enum PlacementError {
    #[error(
        "{0:?} is not a skill name MassOrca places: lowercase letters, digits and hyphens, at most 64"
    )]
    InvalidName(String),
    #[error("choose at least one agent")]
    NoAgents,
    #[error("a skill named {0} is already at {1} and MassOrca did not place it as it is")]
    NameTaken(String, PathBuf),
    #[error("{0} was placed with other content; remove it before placing this copy")]
    ContentChanged(String),
    #[error("MassOrca did not place {0}")]
    NotPlaced(String),
    #[error("{path} cannot be placed: {reason}")]
    Unsupported { path: PathBuf, reason: &'static str },
    #[error("{message}: {path}")]
    Io {
        message: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("the placement record is unreadable: {0}")]
    InvalidRecord(String),
    #[error(transparent)]
    Store(#[from] crate::error::Error),
}
