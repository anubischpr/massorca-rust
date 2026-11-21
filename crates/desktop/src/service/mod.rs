//! The desktop's link to the MassOrca service (decision 0028): the connection states, the updates
//! the UI folds into its store, and the typed refusals that pick a dialog.

mod connection;
mod refusal;
mod update;

pub use connection::{
    BACKOFF_MAX, BACKOFF_START, Backoff, ConnectionState, after_failed_attempt, after_loss,
    reason_text,
};
pub use refusal::Refusal;
pub use update::{StoreState, StoreUpdate};
