//! The client of the MassOrca service: connects to a profile's endpoint, starts the service when allowed, and makes typed
//! calls and subscriptions over one connection (ADR 0002, 0003).

mod client;
mod connect;
mod device_calls;
mod device_name;
mod error;
mod git_calls;
mod handshake;
mod hosts;
mod options;
mod pump;
mod remote;
mod secret_calls;
mod spawn;
mod subscription;
mod team_calls;
mod terminal_attach;
mod transport;
mod typed_calls;
#[cfg(windows)]
mod windows_handle_inheritance;

pub use client::Client;
pub use device_name::default_device_name;
pub use error::{ClientError, profile_refusal};
pub use handshake::Welcome;
pub use hosts::{HostRecord, HostRemoved, Hosts, validate_host_name};
pub use options::{Autostart, ConnectOptions};
pub use subscription::{StreamItem, Subscription};
pub use terminal_attach::{AttachUpdate, TerminalAttachment};
pub use transport::{FrameIo, FrameQueues};
