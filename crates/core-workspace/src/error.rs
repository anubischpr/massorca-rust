use std::fmt;

use massorca_core_git::GitError;
use massorca_core_store::StoreError;
use massorca_protocol::{ErrorBody, codes};

/// A refused or failed domain operation: a stable protocol code and a message for humans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainError {
    code: &'static str,
    message: String,
}

impl DomainError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// The stable protocol code, such as `workspace.dirty`. Clients branch on it.
    pub fn code(&self) -> &'static str {
        self.code
    }

    /// Human-readable detail; never parse it.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for DomainError {}

impl From<DomainError> for ErrorBody {
    fn from(error: DomainError) -> Self {
        ErrorBody::new(error.code, error.message)
    }
}

impl From<StoreError> for DomainError {
    fn from(error: StoreError) -> Self {
        let code = match error {
            StoreError::NewerSchema { .. } => codes::STORE_NEWER_SCHEMA,
            _ => codes::INTERNAL,
        };
        Self::new(code, error.to_string())
    }
}

impl From<GitError> for DomainError {
    fn from(error: GitError) -> Self {
        let code = match error {
            GitError::NotFound | GitError::Spawn(_) => codes::GIT_UNAVAILABLE,
            GitError::Timeout { .. }
            | GitError::Failed { .. }
            | GitError::InvalidOutput { .. }
            | GitError::File { .. } => codes::GIT_FAILED,
            // The caller named a path git cannot record; nothing ran.
            GitError::UnsupportedPath { .. } => codes::REQUEST_INVALID_PARAMS,
        };
        Self::new(code, error.to_string())
    }
}
