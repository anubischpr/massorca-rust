//! Open connections of each paired device, so revoking a device closes them at once (decision 0041).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use massorca_protocol::{ErrorBody, codes};
use tokio::sync::watch;

/// Per device id: each open connection's sequence number and its revocation signal.
type Connections = HashMap<String, Vec<(u64, watch::Sender<bool>)>>;

#[derive(Debug, Default)]
pub(crate) struct LiveDevices {
    connections: Mutex<Connections>,
    next: AtomicU64,
}

/// Keeps one connection registered; dropping it unregisters. `revoked` resolves once the device is revoked.
#[derive(Debug)]
pub(crate) struct LiveGuard {
    live: Arc<LiveDevices>,
    device: String,
    seq: u64,
    pub(crate) revoked: watch::Receiver<bool>,
}

impl LiveDevices {
    pub(crate) fn register(self: &Arc<Self>, device: &str) -> LiveGuard {
        let seq = self.next.fetch_add(1, Ordering::Relaxed);
        let (sender, revoked) = watch::channel(false);
        self.lock()
            .entry(device.to_owned())
            .or_default()
            .push((seq, sender));
        LiveGuard {
            live: Arc::clone(self),
            device: device.to_owned(),
            seq,
            revoked,
        }
    }

    /// Registers a connection of `device`, then asks `is_revoked`. Registering first closes the race with
    /// `device.revoke`, which records the revocation before signalling: a revoke that lands after the device row was
    /// read is either signalled to this guard or seen by the check.
    pub(crate) async fn admit<F, Fut>(
        self: &Arc<Self>,
        device: &str,
        is_revoked: F,
    ) -> Result<LiveGuard, ErrorBody>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<bool, ErrorBody>>,
    {
        let guard = self.register(device);
        if is_revoked().await? || *guard.revoked.borrow() {
            return Err(revoked_error());
        }
        Ok(guard)
    }

    /// Signals every open connection of `device` to close. Returns how many there were.
    pub(crate) fn revoke(&self, device: &str) -> usize {
        let senders = self.lock().remove(device).unwrap_or_default();
        for (_, sender) in &senders {
            let _ = sender.send(true);
        }
        senders.len()
    }

    fn lock(&self) -> MutexGuard<'_, Connections> {
        self.connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// The refusal of a revoked device, at connect or when a revocation races the connection.
pub(crate) fn revoked_error() -> ErrorBody {
    ErrorBody::new(
        codes::AUTH_DEVICE_REVOKED,
        "this device's access was revoked; pair it again to reconnect",
    )
}

impl Drop for LiveGuard {
    fn drop(&mut self) {
        let mut connections = self.live.lock();
        if let Some(list) = connections.get_mut(&self.device) {
            list.retain(|(seq, _)| *seq != self.seq);
            if list.is_empty() {
                connections.remove(&self.device);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use super::*;

    #[test]
    fn revoking_signals_only_that_device() {
        let live = Arc::new(LiveDevices::default());
        let a1 = live.register("a");
        let a2 = live.register("a");
        let b = live.register("b");
        assert_eq!(live.revoke("a"), 2);
        assert!(*a1.revoked.borrow());
        assert!(*a2.revoked.borrow());
        assert!(!*b.revoked.borrow());
        drop(b);
        assert_eq!(live.revoke("b"), 0);
    }

    /// The revocation lands after the device row was read as active and before the connection registered, so
    /// `revoke` signals nobody; only the check after registering can refuse the connection.
    #[tokio::test]
    async fn a_revoke_between_authentication_and_registration_is_refused() {
        let live = Arc::new(LiveDevices::default());
        let stored_revoked = AtomicBool::new(false);
        // Authentication read the row: active.
        assert!(!stored_revoked.load(Ordering::SeqCst));
        // `device.revoke` runs now: it records the revocation, then signals the registered connections (none).
        stored_revoked.store(true, Ordering::SeqCst);
        assert_eq!(live.revoke("a"), 0);
        let admitted = live
            .admit("a", || async { Ok(stored_revoked.load(Ordering::SeqCst)) })
            .await;
        assert_eq!(
            admitted.expect_err("refused").code,
            codes::AUTH_DEVICE_REVOKED
        );
        assert_eq!(
            live.revoke("a"),
            0,
            "a refused connection stays unregistered"
        );
    }

    #[tokio::test]
    async fn a_revoke_after_admission_signals_the_connection() {
        let live = Arc::new(LiveDevices::default());
        let guard = live.admit("a", || async { Ok(false) }).await.unwrap();
        assert_eq!(live.revoke("a"), 1);
        assert!(*guard.revoked.borrow());
    }
}
