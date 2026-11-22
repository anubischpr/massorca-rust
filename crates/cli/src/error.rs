//! The failure a command reports: a stable code, a message, optional data and the exit code.

use std::io;

use massorca_client::ClientError;
use massorca_profile::ProfileError;
use massorca_protocol::codes::PROFILE_FOREIGN_ENTRIES;
use massorca_server::ServiceError;
use serde_json::{Value, json};

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
            ClientError::Handshake { error: body, .. } => {
                return Self::from_body(*body, exit_code);
            }
            ClientError::Unavailable(_) => "service.unavailable",
            ClientError::StartFailed(_) => "service.start_failed",
            ClientError::UntrustedEndpoint(_) => "service.untrusted_endpoint",
            ClientError::Disconnected => "service.disconnected",
            ClientError::Protocol(_) => "client.protocol",
            ClientError::Io(_) => "client.io",
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
            ServiceError::ForeignEntries { paths, message } => {
                let paths: Vec<String> = paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect();
                Self {
                    data: Some(Box::new(json!({ "paths": paths }))),
                    ..Self::new(PROFILE_FOREIGN_ENTRIES, message, exit::ERROR)
                }
            }
            other => Self::new("service.failed", other.to_string(), exit::ERROR),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use massorca_server::ServiceError;
    use serde_json::json;

    use super::{CliError, exit};

    #[test]
    fn foreign_entries_report_their_code_and_paths() {
        let planted = PathBuf::from("profiles")
            .join("default")
            .join("planted.json");
        let error = CliError::from(ServiceError::ForeignEntries {
            paths: vec![planted.clone()],
            message: "foreign".to_owned(),
        });

        assert_eq!(error.code, "profile.foreign_entries");
        assert_eq!(error.exit_code, exit::ERROR);
        assert_eq!(
            error.data.as_deref(),
            Some(&json!({ "paths": [planted.display().to_string()] }))
        );
    }
}
