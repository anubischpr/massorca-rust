//! Network transport of MassOrca (decisions 0040, 0041): TLS identities and pins, mutual TLS 1.3, and (next) the
//! WebSocket frame transport. Only this crate handles keys, certificates and TLS configuration.

mod error;
mod identity;
mod pin;
mod tls;

pub use error::NetError;
pub use identity::TlsIdentity;
pub use pin::Pin;
pub use tls::{client_tls_config, server_tls_config};

/// The only path a server upgrades to WebSocket.
pub const WS_PATH: &str = "/massorca/v1";

/// The port a service listens on unless told otherwise.
pub const DEFAULT_PORT: u16 = 7347;
