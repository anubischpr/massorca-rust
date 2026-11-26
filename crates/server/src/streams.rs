//! The streams one connection has open: their cancel handles, and the forwarder that pushes each stream's items.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use massorca_protocol::{ErrorBody, Frame, Message, codes, fits};
use tokio::sync::{mpsc, oneshot};

use crate::method::StreamItem;

/// The open streams of one connection. Removing a stream drops its cancel sender, which stops its forwarder.
#[derive(Clone, Default)]
pub(crate) struct StreamTable {
    inner: Arc<Mutex<Open>>,
}

#[derive(Default)]
struct Open {
    last_id: u64,
    cancels: HashMap<u64, oneshot::Sender<()>>,
    /// Where the client's bytes for a duplex stream go.
    inbound: HashMap<u64, mpsc::Sender<Vec<u8>>>,
}

/// What became of stream bytes a client sent.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Delivery {
    Delivered,
    /// No open duplex stream has the id, or its handler stopped reading: the bytes are dropped.
    Dropped,
    /// The stream's queue is full: the stream has been released and must be told why.
    Overflow,
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

    /// Like `open`, for a stream whose client also sends bytes, which go to `inbound`.
    pub(crate) fn open_duplex(
        &self,
        inbound: mpsc::Sender<Vec<u8>>,
    ) -> (u64, oneshot::Receiver<()>) {
        let (id, cancelled) = self.open();
        lock(&self.inner).inbound.insert(id, inbound);
        (id, cancelled)
    }

    /// Hands a client's stream bytes to their duplex stream without waiting, so one slow stream never stalls the
    /// connection's reader.
    pub(crate) fn deliver(&self, stream: u64, payload: Vec<u8>) -> Delivery {
        let mut open = lock(&self.inner);
        let Some(inbound) = open.inbound.get(&stream) else {
            return Delivery::Dropped;
        };
        match inbound.try_send(payload) {
            Ok(()) => Delivery::Delivered,
            Err(mpsc::error::TrySendError::Closed(_)) => Delivery::Dropped,
            Err(mpsc::error::TrySendError::Full(_)) => {
                open.inbound.remove(&stream);
                open.cancels.remove(&stream);
                Delivery::Overflow
            }
        }
    }

    /// Ends a stream, if it is still open: a client's cancel, or a stream that has finished.
    pub(crate) fn release(&self, stream: u64) {
        let mut open = lock(&self.inner);
        open.cancels.remove(&stream);
        open.inbound.remove(&stream);
    }

    pub(crate) fn downgrade(&self) -> WeakStreamTable {
        WeakStreamTable {
            inner: Arc::downgrade(&self.inner),
        }
    }

    /// Ends every open stream and returns their ids.
    pub(crate) fn close_all(&self) -> Vec<u64> {
        let mut open = lock(&self.inner);
        open.inbound.clear();
        open.cancels.drain().map(|(id, _cancel)| id).collect()
    }
}

impl WeakStreamTable {
    pub(crate) fn release(&self, stream: u64) {
        if let Some(inner) = self.inner.upgrade() {
            let mut open = lock(&inner);
            open.cancels.remove(&stream);
            open.inbound.remove(&stream);
        }
    }
}

/// Pushes one stream's items as `event` frames (bytes as stream-bytes frames) and closes it with `end`. A cancel, or a connection that is gone, stops it silently.
pub(crate) async fn forward<T>(
    stream: u64,
    mut source: mpsc::Receiver<T>,
    outbound: mpsc::Sender<Frame>,
    mut cancelled: oneshot::Receiver<()>,
    table: WeakStreamTable,
    max_frame_len: u32,
) where
    T: Into<StreamItem> + Send + 'static,
{
    loop {
        // Cancellation is checked first, so a cancelled stream never sends another item.
        let item = tokio::select! {
            biased;
            _ = &mut cancelled => return,
            item = source.recv() => item,
        };
        let Some(item) = item else {
            table.release(stream);
            let _ = outbound
                .send(Frame::Message(Message::End {
                    stream,
                    error: None,
                }))
                .await;
            return;
        };
        let item: StreamItem = item.into();
        let frame = match item {
            StreamItem::Event(data) => Frame::Message(Message::Event { stream, data }),
            StreamItem::Bytes(payload) => Frame::StreamBytes { stream, payload },
        };
        // An item that cannot be framed ends its stream; the client would otherwise see the connection drop.
        if !fits(&frame, max_frame_len) {
            table.release(stream);
            let error = ErrorBody::new(
                codes::STREAM_ITEM_TOO_LARGE,
                "a stream item is larger than one frame may hold",
            );
            let _ = outbound
                .send(Frame::Message(Message::End {
                    stream,
                    error: Some(error),
                }))
                .await;
            return;
        }
        let sent = tokio::select! {
            biased;
            _ = &mut cancelled => return,
            sent = outbound.send(frame) => sent,
        };
        if sent.is_err() {
            return;
        }
    }
}

fn lock<T>(table: &Mutex<T>) -> MutexGuard<'_, T> {
    table.lock().unwrap_or_else(PoisonError::into_inner)
}
