//! Shutdown: the handle that asks a service to stop, and the signal its tasks wait on.

use std::sync::Arc;

use tokio::sync::watch;

/// Asks the service to stop. It stops accepting, lets in-flight requests finish within a grace period, ends open streams and releases its profile. Clones share the signal, and asking twice is harmless.
#[derive(Clone, Debug)]
pub struct ShutdownHandle {
    signal: Arc<watch::Sender<bool>>,
}

impl ShutdownHandle {
    pub fn shutdown(&self) {
        // The previous value is not needed; the signal stays set either way.
        let _ = self.signal.send_replace(true);
    }
}

/// Resolves once shutdown has been requested.
#[derive(Clone, Debug)]
pub(crate) struct ShutdownSignal {
    receiver: watch::Receiver<bool>,
}

pub(crate) fn channel() -> (ShutdownHandle, ShutdownSignal) {
    let (sender, receiver) = watch::channel(false);
    (
        ShutdownHandle {
            signal: Arc::new(sender),
        },
        ShutdownSignal { receiver },
    )
}

impl ShutdownSignal {
    /// Waits until shutdown is requested; returns at once when it already was.
    pub(crate) async fn wait(&mut self) {
        // The sender lives as long as the service, so a closed channel means the service is already going away.
        let _ = self.receiver.wait_for(|requested| *requested).await;
    }
}
