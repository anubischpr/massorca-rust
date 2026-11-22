//! The desktop's link to the MassOrca service (decision 0028): a bridge that owns the tokio
//! runtime, the connection states, the updates the UI folds into its store, and the typed
//! refusals that pick a dialog.

mod agents;
mod bridge;
mod connection;
mod git;
mod launch;
mod refusal;
mod terminals;
mod update;

#[cfg(test)]
mod git_tests;

#[cfg(test)]
pub(crate) use bridge::{Command, Script};
pub use bridge::{ServiceBridge, ServiceHandle, connect_options};
pub use connection::{
    BACKOFF_MAX, BACKOFF_START, Backoff, ConnectionState, after_failed_attempt, after_loss,
    reason_text,
};
pub(crate) use launch::truthy;
pub use launch::{LaunchOptions, autostart_next_to};
pub use refusal::Refusal;
pub use terminals::{AttachRole, StreamEnd, TerminalStream, TerminalStreamItem, feed_events};
#[cfg(test)]
pub(crate) use terminals::{TerminalCommand, TerminalWrite};
pub use update::{GitUpdate, StoreState, StoreUpdate, WorkspaceGit};

#[cfg(test)]
pub(crate) use update::fixtures;
