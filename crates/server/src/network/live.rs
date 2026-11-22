//! Open connections of each paired device, so revoking a device closes them at once (decision 0041).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

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
}
