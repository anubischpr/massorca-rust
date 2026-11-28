//! The client of the MassOrca service: connects to a profile's endpoint, starts the service when allowed, and makes typed
//! calls and subscriptions over one connection (ADR 0002, 0003).

mod automation_calls;
mod calendar_calls;
mod client;
#[cfg(feature = "local")]
mod connect;
mod device_calls;
mod device_name;
mod error;
mod extension_calls;
mod forward_calls;
mod git_calls;
mod handshake;
mod host_link;
mod host_storage;
mod hosted_service_calls;
#[cfg(feature = "local")]
mod hosts;
mod identity_mismatch;
#[cfg(feature = "local")]
mod options;
mod pump;
mod reconnect;
mod remote;
mod secret_calls;
mod skill_calls;
#[cfg(feature = "local")]
mod spawn;
#[cfg(feature = "local")]
mod ssh_dial;
mod subscription;
mod team_calls;
mod terminal_attach;
mod transport;
mod typed_calls;
#[cfg(all(windows, feature = "local"))]
mod windows_handle_inheritance;

pub use client::Client;
pub use device_name::default_device_name;
pub use error::ClientError;
#[cfg(feature = "local")]
pub use error::profile_refusal;
pub use forward_calls::{ForwardEnd, ForwardHandle};
pub use handshake::Welcome;
pub use host_link::{HostDial, LiveHost, LiveState, LiveWatch};
pub use host_storage::{HostRecord, HostRemoved, HostStorage, SshRoute, validate_host_name};
#[cfg(feature = "local")]
pub use hosts::{DeviceRevocation, Hosts, SshInstall};
pub use identity_mismatch::{IDENTITY_MISMATCH_CODE, IdentityMismatch, TrustedFrom};
pub use massorca_net::{SpliceEnd, SpliceReport};
#[cfg(feature = "local")]
pub use options::{Autostart, ConnectOptions};
pub use reconnect::{
    AttachStep, Backoff, Connect, Link, RECONNECT_MAX, RECONNECT_RESET_AFTER, RECONNECT_START,
    Reopen, Resilient, ResilientAttach, ResilientSubscription, is_transient, jittered,
};
pub use subscription::{StreamItem, Subscription};
pub use terminal_attach::{AttachUpdate, TerminalAttachment};
pub use transport::{FrameIo, FrameQueues};
