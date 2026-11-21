//! Work the service has started and not finished: request handlers and store calls. Shutdown waits until none is left,
//! so the profile is not released while a handler or a store call can still run.

use std::sync::Arc;

use tokio::sync::watch;

/// Counts units of work. Clones share the count.
#[derive(Clone, Debug)]
pub(crate) struct Activity {
    count: Arc<watch::Sender<usize>>,
}

impl Default for Activity {
    fn default() -> Self {
        let (count, _) = watch::channel(0);
        Self {
            count: Arc::new(count),
        }
    }
}

impl Activity {
    /// Counts one unit of work until the returned guard is dropped, which also happens when its task is aborted.
    pub(crate) fn enter(&self) -> ActivityGuard {
        self.count.send_modify(|count| *count += 1);
        ActivityGuard {
            count: Arc::clone(&self.count),
        }
    }

    /// Waits until no unit of work is counted.
    pub(crate) async fn idle(&self) {
        let mut count = self.count.subscribe();
        // The sender lives as long as the service, so the wait ends only when the count reaches zero.
        let _ = count.wait_for(|count| *count == 0).await;
    }
}

/// Keeps one unit of work counted; dropping it ends the unit.
#[derive(Debug)]
pub(crate) struct ActivityGuard {
    count: Arc<watch::Sender<usize>>,
}

impl Drop for ActivityGuard {
    fn drop(&mut self) {
        self.count.send_modify(|count| *count -= 1);
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::Activity;

    #[tokio::test]
    async fn idle_returns_only_after_the_last_unit_ends() {
        let activity = Activity::default();
        let unit = activity.enter();
        let started = Instant::now();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            drop(unit);
        });

        activity.idle().await;
        assert!(started.elapsed() >= Duration::from_millis(100));
    }

    #[tokio::test]
    async fn idle_returns_at_once_when_nothing_is_counted() {
        Activity::default().idle().await;
    }
}
