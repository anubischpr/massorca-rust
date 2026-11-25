//! Errors, each with a stable code. The server maps them onto the protocol.

use std::io;
use std::path::{Path, PathBuf};

use crate::host::ProgramSearch;

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
    pub const RESERVED_PUBLISHER: &str = "extension.reserved_publisher";
    pub const BUSY: &str = "extension.busy";
    pub const VENDOR_COMMAND_REFUSED: &str = "extension.vendor_command_refused";
    pub const UP_TO_DATE: &str = "extension.up_to_date";
}

/// What a remove guard or cleanup returns when it refuses or fails, such as `agent.busy`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
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

/// What a failed vendor command was looked for and where, and the last lines of its log (decision 0092).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diagnosis {
    /// The program names the command looked for, the first word of each command.
    pub programs: Vec<String>,
    pub search: ProgramSearch,
    pub output_tail: Vec<String>,
}

/// The `data` of a failed vendor command. `searched` and `output_tail` name host paths, so the server strips them from
/// a caller without host administration.
fn diagnosis_data(agent: &str, diagnosis: &Diagnosis) -> serde_json::Value {
    serde_json::json!({
        "agent": agent,
        "programs": diagnosis.programs,
        "path_source": diagnosis.search.source,
        "searched": diagnosis.search.dirs,
        "output_tail": diagnosis.output_tail,
    })
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
    #[error(
        "{id} from {reference} declares a vendor command; only the built-in catalogue may run one"
    )]
    VendorCommandRefused { id: String, reference: String },
    #[error("extension {0} is busy: its install or removal is still running")]
    Busy(String),
    #[error("extension {0} is already installed")]
    AlreadyInstalled(String),
    #[error("extension {0} is already up to date")]
    UpToDate(String),
    #[error("the source of {id} now holds {found}; remove {id} and install {found} instead")]
    SourceIdChanged { id: String, found: String },
    #[error("agent kind {kind} is already installed by {installed_by}")]
    KindConflict { kind: String, installed_by: String },
    #[error("a package archive needs its SHA-256")]
    HashRequired,
    #[error(
        "removing runs a vendor uninstall command; accept it with --yes or --uninstall-consent {fingerprint}"
    )]
    UninstallConsentRequired {
        commands: Vec<Vec<String>>,
        fingerprint: String,
    },
    #[error("the install review was not accepted")]
    ConsentRequired,
    #[error("the package or its install plan changed since the review")]
    ConsentMismatch,
    #[error("the install runs a vendor installer, which needs its own consent")]
    InstallerConsentRequired,
    #[error("the installer for {agent} failed: {message}")]
    InstallerFailed {
        agent: String,
        message: String,
        diagnosis: Box<Diagnosis>,
    },
    #[error("the {agent} CLI did not verify after its installer: {message}{}", if .installer.is_some() { "; the installer ran, so the CLI it installed may remain" } else { "" })]
    VerifyFailed {
        agent: String,
        message: String,
        /// The vendor command that ran and succeeded before verify failed, when one did.
        installer: Option<Vec<String>>,
        diagnosis: Box<Diagnosis>,
    },
    #[error("removal refused by {owner}: {}", .error.message)]
    Refused { owner: String, error: HookError },
    #[error("{0} is not a file the listing names")]
    AssetNotListed(String),
    #[error("{path} is {size} bytes, more than the {limit} allowed")]
    AssetTooLarge { path: String, size: u64, limit: u64 },
    #[error("invalid lockfile: {0}")]
    InvalidLockfile(String),
    #[error(
        "{id} is reserved to the built-in catalogue: publisher massorca and massorca.* ids are refused from {reference}"
    )]
    ReservedIdentity { id: String, reference: String },
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
            Self::UpToDate(_) => codes::UP_TO_DATE,
            Self::SourceIdChanged { .. } => codes::INVALID_SOURCE,
            Self::Busy(_) => codes::BUSY,
            Self::VendorCommandRefused { .. } => codes::VENDOR_COMMAND_REFUSED,
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
            Self::ReservedIdentity { .. } => codes::RESERVED_PUBLISHER,
            Self::UninstallConsentRequired { .. } => codes::CONSENT_REQUIRED,
            Self::Io { .. } => codes::IO,
        }
    }

    /// What a failed vendor command was looked for and where, when this failure is one.
    pub fn diagnosis(&self) -> Option<&Diagnosis> {
        match self {
            Self::InstallerFailed { diagnosis, .. } | Self::VerifyFailed { diagnosis, .. } => {
                Some(diagnosis.as_ref())
            }
            _ => None,
        }
    }

    /// Structured detail for the protocol error's `data`: after a vendor installer ran and verify failed, the command
    /// that ran and that its CLI may remain on the host.
    pub fn data(&self) -> Option<serde_json::Value> {
        match self {
            Self::UninstallConsentRequired {
                commands,
                fingerprint,
            } => Some(serde_json::json!({ "uninstall": commands, "fingerprint": fingerprint })),
            Self::InstallerFailed {
                agent, diagnosis, ..
            } => Some(diagnosis_data(agent, diagnosis)),
            Self::VerifyFailed {
                agent,
                installer,
                diagnosis,
                ..
            } => {
                let mut data = diagnosis_data(agent, diagnosis);
                if let Some(command) = installer {
                    data["installer_ran"] = serde_json::json!(true);
                    data["cli_may_remain"] = serde_json::json!(true);
                    data["command"] = serde_json::json!(command);
                }
                Some(data)
            }
            _ => None,
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

    #[test]
    fn an_update_with_nothing_new_has_its_own_code() {
        assert_eq!(
            Error::UpToDate("acme.kit".to_owned()).code(),
            codes::UP_TO_DATE
        );
    }
}
