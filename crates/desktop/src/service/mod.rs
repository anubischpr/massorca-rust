//! The desktop's link to the MassOrca service (decision 0028): a bridge that owns the tokio
//! runtime, the connection states, the updates the UI folds into its store, and the typed
//! refusals that pick a dialog.

mod bridge;
mod connection;
mod launch;
mod refusal;
mod update;

pub use bridge::{ServiceBridge, ServiceHandle, connect_options};
pub use connection::{
    BACKOFF_MAX, BACKOFF_START, Backoff, ConnectionState, after_failed_attempt, after_loss,
    reason_text,
};
pub use launch::{LaunchOptions, autostart_next_to};
pub use refusal::Refusal;
pub use update::{StoreState, StoreUpdate};
