//! Why the service could not start, or stopped abnormally.

use std::io;
use std::path::PathBuf;

use massorca_core_store::StoreError;
use massorca_profile::RootAccessError;

use crate::method::MethodError;
use crate::registry::RegistrationError;

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("another service already runs this profile")]
    AlreadyRunning,
    #[error("service i/o failed: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Registration(#[from] RegistrationError),
    #[error(transparent)]
    Method(#[from] MethodError),
    #[error("cannot pick the default workspace root: {0}")]
    DefaultWorkspaceRoot(String),
    /// The profile directory, or entries below it, belong to another user (ADR 0073). Up to ten paths.
    #[error("{message}")]
    ForeignEntries {
        paths: Vec<PathBuf>,
        message: String,
    },
}

impl From<RootAccessError> for ServiceError {
    fn from(error: RootAccessError) -> Self {
        match error {
            RootAccessError::Io(error) => Self::Io(error),
            RootAccessError::ForeignEntries { ref paths } => Self::ForeignEntries {
                paths: paths.clone(),
                message: error.to_string(),
            },
        }
    }
}
