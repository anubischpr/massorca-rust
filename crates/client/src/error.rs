//! Client failures, and the CLI exit code each one maps to: 1 for a service error, 4 when the service cannot be reached.

use std::io;

use massorca_protocol::{ErrorBody, ProtocolRange};

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("the service is not reachable: {0}")]
    Unavailable(#[source] io::Error),
    #[error("could not start the service: {0}")]
    StartFailed(String),
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
    #[error(
        "{0:?} is not a host name: 1 to 32 of a-z, 0-9, - and _, starting with a letter or digit, and not \"local\""
    )]
    InvalidHostName(String),
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
            | Self::InvalidHostName(_) => 1,
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
