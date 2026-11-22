//! The client of the MassOrca service: connects to a profile's endpoint, starts the service when allowed, and makes typed
//! calls and subscriptions over one connection (ADR 0002, 0003).

mod client;
mod connect;
mod error;
mod git_calls;
mod handshake;
mod options;
mod pump;
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
pub use options::{Autostart, ConnectOptions};
pub use subscription::{StreamItem, Subscription};
pub use terminal_attach::{AttachUpdate, TerminalAttachment};
pub use transport::{FrameIo, FrameQueues};
