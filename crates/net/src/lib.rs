//! Network transport of MassOrca (decisions 0040, 0041): TLS identities and pins, mutual TLS 1.3, and the WebSocket
//! frame transport. Only this crate handles keys, certificates and TLS configuration.

mod error;
mod frames;
mod identity;
mod log_guard;
mod log_redaction;
mod pin;
mod tls;
mod transport;

pub use error::NetError;
pub use frames::{
    Keepalive, PreAuthGate, PreAuthOpener, pre_auth_gate, spawn_frame_tasks,
    spawn_gated_frame_tasks,
};
pub use identity::TlsIdentity;
pub use log_guard::{FRAME_DUMPING_TARGETS, frame_dump_guard};
pub use log_redaction::{REDACTED, RedactedFormat, redact_secret_values};
pub use pin::Pin;
pub use tls::{client_tls_config, server_tls_config};
pub use transport::{Accepted, ClientWs, ServerWs, accept, connect};

/// The only path a server upgrades to WebSocket.
pub const WS_PATH: &str = "/massorca/v1";

/// The port a service listens on unless told otherwise.
pub const DEFAULT_PORT: u16 = 7347;
