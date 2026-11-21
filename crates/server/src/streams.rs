//! The streams one connection has open: their cancel handles, and the forwarder that pushes each stream's items.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use massorca_protocol::Message;
use tokio::sync::{mpsc, oneshot};

use crate::method::StreamSource;

/// The open streams of one connection. Removing a stream drops its cancel sender, which stops its forwarder.
#[derive(Clone, Default)]
pub(crate) struct StreamTable {
    inner: Arc<Mutex<Open>>,
}

#[derive(Default)]
struct Open {
    last_id: u64,
    cancels: HashMap<u64, oneshot::Sender<()>>,
}

/// A table handle a forwarder can release its own stream through, without keeping the table alive.
#[derive(Clone)]
pub(crate) struct WeakStreamTable {
    inner: Weak<Mutex<Open>>,
}

impl StreamTable {
    /// Allocates the next stream id and the receiver its forwarder watches for cancellation.
    pub(crate) fn open(&self) -> (u64, oneshot::Receiver<()>) {
        let mut open = lock(&self.inner);
        open.last_id += 1;
        let id = open.last_id;
        let (cancel, cancelled) = oneshot::channel();
        open.cancels.insert(id, cancel);
        (id, cancelled)
    }

    /// Ends a stream, if it is still open: a client's cancel, or a stream that has finished.
    pub(crate) fn release(&self, stream: u64) {
        lock(&self.inner).cancels.remove(&stream);
    }

    pub(crate) fn downgrade(&self) -> WeakStreamTable {
        WeakStreamTable {
            inner: Arc::downgrade(&self.inner),
        }
    }

    /// Ends every open stream and returns their ids.
    pub(crate) fn close_all(&self) -> Vec<u64> {
        lock(&self.inner)
            .cancels
            .drain()
            .map(|(id, _cancel)| id)
            .collect()
    }
}

impl WeakStreamTable {
    pub(crate) fn release(&self, stream: u64) {
        if let Some(inner) = self.inner.upgrade() {
            lock(&inner).cancels.remove(&stream);
        }
    }
}

/// Pushes one stream's items as `event` frames and closes it with `end`. A cancel, or a connection that is gone, stops it silently.
pub(crate) async fn forward(
    stream: u64,
    mut source: StreamSource,
    outbound: mpsc::Sender<Message>,
    mut cancelled: oneshot::Receiver<()>,
    table: WeakStreamTable,
) {
    loop {
        // Cancellation is checked first, so a cancelled stream never sends another item.
        let item = tokio::select! {
            biased;
            _ = &mut cancelled => return,
            item = source.recv() => item,
        };
        let Some(data) = item else {
            table.release(stream);
            let _ = outbound
                .send(Message::End {
                    stream,
                    error: None,
                })
                .await;
            return;
        };
        let sent = tokio::select! {
            biased;
            _ = &mut cancelled => return,
            sent = outbound.send(Message::Event { stream, data }) => sent,
        };
        if sent.is_err() {
            return;
        }
    }
}

fn lock<T>(table: &Mutex<T>) -> MutexGuard<'_, T> {
    table.lock().unwrap_or_else(PoisonError::into_inner)
}
