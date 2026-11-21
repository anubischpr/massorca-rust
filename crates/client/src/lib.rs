//! The client of the MassOrca service: connects to a profile's endpoint, starts the service when allowed, and makes typed
//! calls and subscriptions over one connection (ADR 0002, 0003).

mod client;
mod connect;
mod error;
mod handshake;
mod hosts;
mod options;
mod pump;
mod remote;
mod spawn;
mod subscription;
mod terminal_attach;
mod transport;
mod typed_calls;
#[cfg(windows)]
mod windows_handle_inheritance;

pub use client::Client;
pub use error::ClientError;
pub use handshake::Welcome;
pub use hosts::{HostRecord, Hosts, validate_host_name};
pub use options::{Autostart, ConnectOptions};
pub use subscription::{StreamItem, Subscription};
pub use terminal_attach::{AttachUpdate, TerminalAttachment};
pub use transport::{FrameIo, FrameQueues};
