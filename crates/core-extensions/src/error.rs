//! Errors, each with a stable code. The server maps them onto the protocol.

use std::io;
use std::path::{Path, PathBuf};

/// Stable codes under the reserved `extension.` prefix. Package-format errors keep their codes from
/// `massorca_ext_package::codes`.
pub mod codes {
    pub const INVALID_SOURCE: &str = "extension.invalid_source";
    pub const NOT_FOUND: &str = "extension.not_found";
    pub const NOT_INSTALLED: &str = "extension.not_installed";
    pub const ALREADY_INSTALLED: &str = "extension.already_installed";
    pub const KIND_CONFLICT: &str = "extension.kind_conflict";
    pub const HASH_REQUIRED: &str = "extension.hash_required";
    pub const CONSENT_REQUIRED: &str = "extension.consent_required";
    pub const CONSENT_MISMATCH: &str = "extension.consent_mismatch";
    pub const INSTALLER_CONSENT_REQUIRED: &str = "extension.installer_consent_required";
    pub const INSTALLER_FAILED: &str = "extension.installer_failed";
    pub const VERIFY_FAILED: &str = "extension.verify_failed";
    pub const ASSET_NOT_LISTED: &str = "extension.asset_not_listed";
    pub const ASSET_TOO_LARGE: &str = "extension.asset_too_large";
    pub const INVALID_LOCKFILE: &str = "extension.invalid_lockfile";
    pub const IO: &str = "extension.io";
    pub const TAMPERED: &str = "extension.tampered";
}

/// What a remove guard or cleanup returns when it refuses or fails, such as `agent.busy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookError {
    pub code: String,
    pub message: String,
}

impl HookError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Everything the extension service refuses or fails at.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Package(#[from] massorca_ext_package::Error),
    #[error("invalid source {0:?}: expected catalogue:<id>, dir:<path> or file:<path>")]
    InvalidSource(String),
    #[error("no catalogue entry {0}")]
    NotFound(String),
    #[error("extension {0} is not installed")]
    NotInstalled(String),
    #[error("extension {0} is already installed")]
    AlreadyInstalled(String),
    #[error("agent kind {kind} is already installed by {installed_by}")]
    KindConflict { kind: String, installed_by: String },
    #[error("a package archive needs its SHA-256")]
    HashRequired,
    #[error("the install review was not accepted")]
    ConsentRequired,
    #[error("the package or its install plan changed since the review")]
    ConsentMismatch,
    #[error("the install runs a vendor installer, which needs its own consent")]
    InstallerConsentRequired,
    #[error("the installer for {agent} failed: {message}")]
    InstallerFailed { agent: String, message: String },
    #[error("the {agent} CLI did not verify after its installer: {message}")]
    VerifyFailed { agent: String, message: String },
    #[error("removal refused by {owner}: {}", .error.message)]
    Refused { owner: String, error: HookError },
    #[error("{0} is not a file the listing names")]
    AssetNotListed(String),
    #[error("{path} is {size} bytes, more than the {limit} allowed")]
    AssetTooLarge { path: String, size: u64, limit: u64 },
    #[error("invalid lockfile: {0}")]
    InvalidLockfile(String),
    #[error("the installed files of {0} differ from what was reviewed; reinstall it")]
    Tampered(String),
    #[error("{context} {}: {source}", .path.display())]
    Io {
        context: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl Error {
    /// The protocol code. A refusal carries the guard's own code, such as `agent.busy`.
    pub fn code(&self) -> &str {
        match self {
            Self::Package(error) => error.code(),
            Self::InvalidSource(_) => codes::INVALID_SOURCE,
            Self::NotFound(_) => codes::NOT_FOUND,
            Self::NotInstalled(_) => codes::NOT_INSTALLED,
            Self::AlreadyInstalled(_) => codes::ALREADY_INSTALLED,
            Self::KindConflict { .. } => codes::KIND_CONFLICT,
            Self::HashRequired => codes::HASH_REQUIRED,
            Self::ConsentRequired => codes::CONSENT_REQUIRED,
            Self::ConsentMismatch => codes::CONSENT_MISMATCH,
            Self::InstallerConsentRequired => codes::INSTALLER_CONSENT_REQUIRED,
            Self::InstallerFailed { .. } => codes::INSTALLER_FAILED,
            Self::VerifyFailed { .. } => codes::VERIFY_FAILED,
            Self::Refused { error, .. } => &error.code,
            Self::AssetNotListed(_) => codes::ASSET_NOT_LISTED,
            Self::AssetTooLarge { .. } => codes::ASSET_TOO_LARGE,
            Self::InvalidLockfile(_) => codes::INVALID_LOCKFILE,
            Self::Tampered(_) => codes::TAMPERED,
            Self::Io { .. } => codes::IO,
        }
    }

    /// Wraps an I/O error with what was being done and where.
    pub(crate) fn io(context: &'static str, path: &Path) -> impl FnOnce(io::Error) -> Self {
        let path = path.to_path_buf();
        move |source| Self::Io {
            context,
            path,
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_carries_the_guard_code() {
        let error = Error::Refused {
            owner: "agents".to_owned(),
            error: HookError::new("agent.busy", "a session is live"),
        };
        assert_eq!(error.code(), "agent.busy");
        assert_eq!(Error::ConsentRequired.code(), codes::CONSENT_REQUIRED);
    }
}
