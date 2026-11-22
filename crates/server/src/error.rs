//! Why the service could not start, or stopped abnormally.

use std::io;

use massorca_core_store::StoreError;

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
}
