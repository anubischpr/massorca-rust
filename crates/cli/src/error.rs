//! The failure a command reports: a stable code, a message, optional data and the exit code.

use std::io;

use massorca_client::{ClientError, profile_refusal};
use massorca_core_remote::bootstrap::BootstrapError;
use massorca_core_remote::ssh::SshError;
use massorca_profile::{ProfileError, RootAccessError};
use massorca_protocol::codes;
use massorca_protocol::pairing::OfferError;
use massorca_server::{ListenSpecError, ServiceError};
use serde_json::Value;

/// Exit codes of `massorca` (docs/architecture/core.md).
pub mod exit {
    /// The service answered with an error, or local data could not be used.
    pub const ERROR: u8 = 1;
    /// The command line is invalid.
    pub const USAGE: u8 = 2;
    /// `service run`: another service owns the profile.
    pub const ALREADY_RUNNING: u8 = 3;
    /// The service could not be reached or started.
    pub const UNAVAILABLE: u8 = 4;
}

/// A failed command: the code and message to report, optional data, and the exit code.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{message} ({code})")]
pub struct CliError {
    pub code: String,
    pub message: String,
    /// Boxed so the error stays small enough for `Result` returns (clippy `result_large_err`).
    pub data: Option<Box<Value>>,
    pub exit_code: u8,
}

impl CliError {
    pub fn new(code: &str, message: impl Into<String>, exit_code: u8) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
            data: None,
            exit_code,
        }
    }

    fn from_body(body: massorca_protocol::ErrorBody, exit_code: u8) -> Self {
        Self {
            code: body.code,
            message: body.message,
            data: body.data.map(Box::new),
            exit_code,
        }
    }
}

impl From<io::Error> for CliError {
    fn from(error: io::Error) -> Self {
        Self::new("io.failed", error.to_string(), exit::ERROR)
    }
}

impl From<ClientError> for CliError {
    fn from(error: ClientError) -> Self {
        let exit_code = u8::try_from(error.exit_code()).unwrap_or(exit::ERROR);
        let message = error.to_string();
        let code = match error {
            ClientError::Remote(body) => return Self::from_body(body, exit_code),
            ClientError::StartRefused(body) | ClientError::ProfileRefused(body) => {
                return Self::from_body(*body, exit_code);
            }
            ClientError::Handshake { error: body, .. } => {
                return Self::from_body(*body, exit_code);
            }
            ClientError::Unavailable(_) => "service.unavailable",
            ClientError::StartFailed(_) => "service.start_failed",
            ClientError::UntrustedEndpoint(_) => "service.untrusted_endpoint",
            ClientError::Disconnected => "service.disconnected",
            ClientError::Protocol(_) => "client.protocol",
            ClientError::Io(_) => "client.io",
            ClientError::HostUnreachable(_) => "host.unreachable",
            ClientError::ServerIdentityMismatch => "host.identity_mismatch",
            ClientError::HostNotFound(_) => "host.not_found",
            ClientError::HostExists(_) => "host.exists",
            ClientError::HostUnreadable { .. } => "host.unreadable",
            ClientError::HostsBusy(_) => "host.busy",
            ClientError::InvalidHostName(_) => "host.invalid_name",
        };
        Self::new(code, message, exit_code)
    }
}

impl From<ProfileError> for CliError {
    fn from(error: ProfileError) -> Self {
        let (code, exit_code) = match error {
            ProfileError::InvalidName { .. } => ("usage.invalid_profile", exit::USAGE),
            ProfileError::InvalidHome { .. } => ("usage.invalid_home", exit::USAGE),
            ProfileError::NoHomeDirectory => ("profile.no_home_directory", exit::ERROR),
        };
        Self::new(code, error.to_string(), exit_code)
    }
}

impl From<ServiceError> for CliError {
    fn from(error: ServiceError) -> Self {
        match error {
            ServiceError::AlreadyRunning => Self::new(
                "service.already_running",
                "another service already runs this profile",
                exit::ALREADY_RUNNING,
            ),
            ServiceError::ProfileAccess(refused) => Self::from(refused),
            other => Self::new("service.failed", other.to_string(), exit::ERROR),
        }
    }
}

impl From<RootAccessError> for CliError {
    fn from(error: RootAccessError) -> Self {
        match profile_refusal(&error) {
            Some(body) => Self::from_body(body, exit::ERROR),
            None => Self::new("io.failed", error.to_string(), exit::ERROR),
        }
    }
}

impl From<ListenSpecError> for CliError {
    fn from(error: ListenSpecError) -> Self {
        Self::new("usage.invalid_listen", error.to_string(), exit::USAGE)
    }
}

/// Never echoes the offer: its messages are generic.
impl From<OfferError> for CliError {
    fn from(error: OfferError) -> Self {
        let code = match &error {
            OfferError::NotAnOffer => "pairing.not_an_offer",
            OfferError::Damaged => "pairing.damaged",
        };
        Self::new(code, error.to_string(), exit::ERROR)
    }
}

impl From<SshError> for CliError {
    fn from(error: SshError) -> Self {
        let (code, exit_code) = match &error {
            SshError::NotFound => ("ssh.not_found", exit::ERROR),
            SshError::Refused(_) => ("usage.invalid_ssh_target", exit::USAGE),
            SshError::Spawn(_) => ("ssh.failed", exit::ERROR),
            SshError::Failed { .. } | SshError::TimedOut(_) => {
                ("host.unreachable", exit::UNAVAILABLE)
            }
        };
        Self::new(code, error.to_string(), exit_code)
    }
}

impl From<BootstrapError> for CliError {
    fn from(error: BootstrapError) -> Self {
        match error {
            BootstrapError::Ssh(inner) => inner.into(),
            BootstrapError::InvalidPlan(message) => {
                Self::new("usage.invalid_plan", message, exit::USAGE)
            }
            BootstrapError::UnsupportedSystem(message) => {
                Self::new("host.unsupported_system", message, exit::ERROR)
            }
            BootstrapError::BinaryMismatch(message) => {
                Self::new("host.binary_mismatch", message, exit::ERROR)
            }
            BootstrapError::Remote(message) => {
                Self::new("host.bootstrap_failed", message, exit::ERROR)
            }
            BootstrapError::Io(error) => error.into(),
        }
    }
}

impl From<tokio::task::JoinError> for CliError {
    fn from(error: tokio::task::JoinError) -> Self {
        Self::new(codes::INTERNAL, error.to_string(), exit::ERROR)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use massorca_profile::RootAccessError;
    use massorca_server::ServiceError;
    use serde_json::json;

    use super::{CliError, exit};

    fn planted() -> PathBuf {
        PathBuf::from("profiles")
            .join("default")
            .join("planted.json")
    }

    #[test]
    fn refused_profiles_report_their_code_and_paths() {
        let cases = [
            (
                RootAccessError::ForeignEntries {
                    paths: vec![planted()],
                },
                "profile.foreign_entries",
                json!({ "paths": [planted().display().to_string()] }),
            ),
            (
                RootAccessError::WritableByOthers {
                    paths: vec![planted()],
                },
                "profile.writable_by_others",
                json!({ "paths": [planted().display().to_string()] }),
            ),
            (
                RootAccessError::ReadableByOthers {
                    paths: vec![planted()],
                },
                "profile.readable_by_others",
                json!({ "paths": [planted().display().to_string()] }),
            ),
            (
                RootAccessError::UnsafeLocation { path: planted() },
                "profile.unsafe_location",
                json!({ "path": planted().display().to_string() }),
            ),
        ];
        for (refused, code, data) in cases {
            let error = CliError::from(ServiceError::from(refused));

            assert_eq!(error.code, code);
            assert_eq!(error.exit_code, exit::ERROR);
            assert_eq!(error.data.as_deref(), Some(&data));
        }
    }

    #[test]
    fn a_profile_writable_by_others_says_why_and_names_profile_secure() {
        let error = CliError::from(ServiceError::from(RootAccessError::WritableByOthers {
            paths: vec![planted()],
        }));

        assert!(
            error.message.contains("writable by other users"),
            "{}",
            error.message
        );
        assert!(
            error
                .message
                .contains("other programs on this computer could change these files")
                && error.message.contains("massorca profile secure"),
            "{}",
            error.message
        );
    }

    #[test]
    fn a_hosts_folder_readable_by_others_says_why_and_what_to_do() {
        let error = CliError::from(ServiceError::from(RootAccessError::ReadableByOthers {
            paths: vec![planted()],
        }));

        assert_eq!(error.code, "profile.readable_by_others");
        assert!(
            error.message.contains(
                "other programs on this computer could read these files, which hold secrets"
            ),
            "{}",
            error.message
        );
        assert!(
            error
                .message
                .contains("remove the folder or restrict it to yourself"),
            "{}",
            error.message
        );
    }
}
