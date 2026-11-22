//! Why the service could not start, or stopped abnormally.

use std::io;

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
    #[error("cannot set up the network listener: {0}")]
    Network(String),
    #[error("cannot pick the default workspace root: {0}")]
    DefaultWorkspaceRoot(String),
    /// The profile location or content is not trusted (ADR 0073); nothing in it was opened.
    #[error(transparent)]
    ProfileAccess(RootAccessError),
}

impl From<RootAccessError> for ServiceError {
    fn from(error: RootAccessError) -> Self {
        match error {
            RootAccessError::Io(error) => Self::Io(error),
            refused => Self::ProfileAccess(refused),
        }
    }
}
