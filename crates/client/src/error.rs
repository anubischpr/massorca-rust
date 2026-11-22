//! Client failures, and the CLI exit code each one maps to: 1 for a service error, 4 when the service cannot be reached.

use std::io;

use massorca_profile::RootAccessError;
use massorca_protocol::{ErrorBody, ProtocolRange, codes};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("the service is not reachable: {0}")]
    Unavailable(#[source] io::Error),
    #[error("could not start the service: {0}")]
    StartFailed(String),
    /// The service this client started refused to start, with its own code and message, for example a profile in an
    /// unsafe location (ADR 0073). Boxed so the error stays small.
    #[error("{} ({})", .0.message, .0.code)]
    StartRefused(Box<ErrorBody>),
    /// The profile is in a place others could change, so this client writes nothing into it (ADR 0073). Its code is
    /// one of `profile.unsafe_location`, `profile.writable_by_others` and `profile.foreign_entries`.
    #[error("{} ({})", .0.message, .0.code)]
    ProfileRefused(Box<ErrorBody>),
    /// The endpoint is served by another user, or its owner cannot be checked. It is never retried.
    #[error("the service endpoint is not trusted: {0} (service.untrusted_endpoint)")]
    UntrustedEndpoint(String),
    /// The service refused the handshake. `supported` is the protocol range it speaks.
    /// Boxed so the error stays small: an `ErrorBody` with `data` makes this variant large.
    #[error("the service refused the connection: {} ({})", .error.message, .error.code)]
    Handshake {
        error: Box<ErrorBody>,
        supported: Option<ProtocolRange>,
    },
    #[error("{} ({})", .0.message, .0.code)]
    Remote(ErrorBody),
    #[error("the connection to the service was lost")]
    Disconnected,
    #[error("unexpected data from the service: {0}")]
    Protocol(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    /// No address of a remote host answered (decision 0046); nothing is known about the work on it.
    #[error("the host is unreachable: {0}")]
    HostUnreachable(String),
    /// The server's key is not the one pinned at pairing. Never retried.
    #[error("the host's identity does not match the one paired with; refusing to connect")]
    ServerIdentityMismatch,
    #[error("no host named {0:?}")]
    HostNotFound(String),
    #[error("a host named {0:?} already exists")]
    HostExists(String),
    /// Whether a host's files are there could not be read, so nothing was changed rather than risk losing a key.
    #[error("the files of host {name} cannot be read, so nothing was changed: {reason}")]
    HostUnreadable { name: String, reason: String },
    #[error(
        "{0:?} is not a host name: 1 to 32 of a-z, 0-9, - and _, starting with a letter or digit, and not \"local\""
    )]
    InvalidHostName(String),
}

impl From<RootAccessError> for ClientError {
    fn from(error: RootAccessError) -> Self {
        match profile_refusal(&error) {
            Some(body) => Self::ProfileRefused(Box::new(body)),
            None => match error {
                RootAccessError::Io(error) => Self::Io(error),
                other => Self::Protocol(other.to_string()),
            },
        }
    }
}

/// The code, message and paths of a refused profile, as the CLI and the desktop show them; `None` for an I/O error.
pub fn profile_refusal(error: &RootAccessError) -> Option<ErrorBody> {
    let shown = |paths: &[std::path::PathBuf]| -> Vec<String> {
        paths
            .iter()
            .map(|path| path.display().to_string())
            .collect()
    };
    let (code, data) = match error {
        RootAccessError::ForeignEntries { paths } => (
            codes::PROFILE_FOREIGN_ENTRIES,
            json!({ "paths": shown(paths) }),
        ),
        RootAccessError::WritableByOthers { paths } => (
            codes::PROFILE_WRITABLE_BY_OTHERS,
            json!({ "paths": shown(paths) }),
        ),
        RootAccessError::ReadableByOthers { paths } => (
            codes::PROFILE_READABLE_BY_OTHERS,
            json!({ "paths": shown(paths) }),
        ),
        RootAccessError::UnsafeLocation { path } => (
            codes::PROFILE_UNSAFE_LOCATION,
            json!({ "path": path.display().to_string() }),
        ),
        RootAccessError::Io(_) => return None,
    };
    Some(ErrorBody {
        code: code.to_owned(),
        message: error.to_string(),
        data: Some(data),
    })
}

impl ClientError {
    /// The CLI exit code (core.md): 1 for a service error or bad local data, 4 when the service cannot be reached or started.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Remote(_)
            | Self::Protocol(_)
            | Self::Io(_)
            | Self::HostNotFound(_)
            | Self::HostExists(_)
            | Self::HostUnreadable { .. }
            | Self::InvalidHostName(_)
            | Self::StartRefused(_)
            | Self::ProfileRefused(_) => 1,
            Self::Unavailable(_)
            | Self::StartFailed(_)
            | Self::UntrustedEndpoint(_)
            | Self::Handshake { .. }
            | Self::Disconnected
            | Self::HostUnreachable(_)
            | Self::ServerIdentityMismatch => 4,
        }
    }
}
