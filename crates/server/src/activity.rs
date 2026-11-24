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

    /// Runs `call` on the blocking pool, counted until it returns even when the awaiting request is aborted.
    pub(crate) async fn blocking<T, F>(&self, call: F) -> Result<T, tokio::task::JoinError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let work = self.enter();
        tokio::task::spawn_blocking(move || {
            let _work = work;
            call()
        })
        .await
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
    async fn a_blocking_call_stays_counted_after_its_request_is_aborted() {
        let activity = Activity::default();
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let (entered, started) = std::sync::mpsc::channel::<()>();
        let runner = activity.clone();
        let request = tokio::spawn(async move {
            runner
                .blocking(move || {
                    let _ = entered.send(());
                    let _ = wait.recv();
                })
                .await
        });
        tokio::task::spawn_blocking(move || started.recv())
            .await
            .expect("joins")
            .expect("the call started");
        request.abort();
        let _ = request.await;

        assert!(
            tokio::time::timeout(Duration::from_millis(100), activity.idle())
                .await
                .is_err(),
            "the running call is still counted"
        );
        release.send(()).expect("the call is released");
        tokio::time::timeout(Duration::from_secs(5), activity.idle())
            .await
            .expect("idle once the call returns");
    }

    #[tokio::test]
    async fn idle_returns_at_once_when_nothing_is_counted() {
        Activity::default().idle().await;
    }
}
