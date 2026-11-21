//! Queued messages: follow-up messages waiting for the agent or the host, shown as a dock above
//! the composer (QueuedMessage.md, X-CH-03). The queue itself belongs to the session; the dock
//! keeps only the transient inline steps.

pub mod state;

pub use state::{
    QueueStatus, QueueView, QueuedItem, QueuedMessagesEvent, RowStep, WaitReason, caption,
    focus_after_removal, header_label,
};
