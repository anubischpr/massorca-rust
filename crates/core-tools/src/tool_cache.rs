//! The last detection of one service run, answered again until a refresh asks for a new one.

use std::sync::{Mutex, PoisonError};

use massorca_protocol::methods::service::ToolInfo;

/// Holds the last detection of this service run.
#[derive(Debug, Default)]
pub struct ToolCache {
    held: Mutex<Option<Vec<ToolInfo>>>,
}

impl ToolCache {
    /// Answers from the held detection, or runs `detect` when there is none or `refresh` is set. Concurrent callers
    /// wait on one detection, so a second caller reads what the first one found.
    pub fn report(&self, refresh: bool, detect: impl FnOnce() -> Vec<ToolInfo>) -> Vec<ToolInfo> {
        let mut held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        if !refresh && let Some(found) = held.as_ref() {
            return found.clone();
        }
        let found = detect();
        *held = Some(found.clone());
        found
    }
}
