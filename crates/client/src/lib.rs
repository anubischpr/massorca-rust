//! The client of the MassOrca service: connects to a profile's endpoint, starts the service when allowed, and makes typed
//! calls and subscriptions over one connection (ADR 0002, 0003).

mod error;
mod handshake;
mod options;
mod spawn;
mod transport;

pub use error::ClientError;
pub use handshake::Welcome;
pub use options::{Autostart, ConnectOptions};
