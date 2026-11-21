//! Shutdown: the handle that asks a service to stop, and the signal its tasks wait on.

use std::sync::Arc;

use tokio::sync::watch;

/// Asks the service to stop. It stops accepting, lets in-flight requests finish within a grace period, ends open streams and releases its profile. Clones share the signal, and asking twice is harmless.
#[derive(Clone, Debug)]
pub struct ShutdownHandle {
    signal: Arc<watch::Sender<bool>>,
    listener_closed: Arc<watch::Sender<bool>>,
}

impl ShutdownHandle {
    pub fn shutdown(&self) {
        // The previous value is not needed; the signal stays set either way.
        let _ = self.signal.send_replace(true);
    }

    /// Waits until the service has stopped accepting connections; returns at once when it already has.
    pub(crate) async fn listener_closed(&self) {
        let mut closed = self.listener_closed.subscribe();
        let _ = closed.wait_for(|closed| *closed).await;
    }
}

/// Resolves once shutdown has been requested, and lets the accept loop report that it stopped listening.
#[derive(Clone, Debug)]
pub(crate) struct ShutdownSignal {
    receiver: watch::Receiver<bool>,
    listener_closed: Arc<watch::Sender<bool>>,
}

pub(crate) fn channel() -> (ShutdownHandle, ShutdownSignal) {
    let (sender, receiver) = watch::channel(false);
    let (listener_closed, _) = watch::channel(false);
    let listener_closed = Arc::new(listener_closed);
    (
        ShutdownHandle {
            signal: Arc::new(sender),
            listener_closed: Arc::clone(&listener_closed),
        },
        ShutdownSignal {
            receiver,
            listener_closed,
        },
    )
}

impl ShutdownSignal {
    /// Waits until shutdown is requested; returns at once when it already was.
    pub(crate) async fn wait(&mut self) {
        // The sender lives as long as the service, so a closed channel means the service is already going away.
        let _ = self.receiver.wait_for(|requested| *requested).await;
    }

    /// Records that the listener is closed, so no connection can be accepted from now on.
    pub(crate) fn mark_listener_closed(&self) {
        let _ = self.listener_closed.send_replace(true);
    }
}
