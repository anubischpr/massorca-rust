//! A guard that aborts a task when it is dropped, so the task stops with the future that started it.

use tokio::task::AbortHandle;

/// Aborts the task it guards when dropped. A dropped `JoinHandle` would leave the task running.
pub(crate) struct AbortOnDrop(AbortHandle);

impl AbortOnDrop {
    pub(crate) fn new(handle: AbortHandle) -> Self {
        Self(handle)
    }
}

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
