//! A stream opened by a call. Dropping it asks the service to stop sending.

use std::fmt;

use massorca_protocol::{ErrorBody, Message, codes};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::error::ClientError;
use crate::pump::Item;

/// One item of a subscription: a JSON event, or raw bytes the service sent as a stream-bytes frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamItem {
    Event(Value),
    Bytes(Vec<u8>),
}

/// The items of one stream. They queue until read, up to a limit: a subscriber that falls behind gets `stream.overflow`.
/// Dropping the subscription cancels the stream on the service.
pub struct Subscription {
    stream: u64,
    items: mpsc::Receiver<Item>,
    outbound: mpsc::Sender<Message>,
    finished: bool,
}

impl Subscription {
    pub(crate) fn new(
        stream: u64,
        items: mpsc::Receiver<Item>,
        outbound: mpsc::Sender<Message>,
    ) -> Self {
        Self {
            stream,
            items,
            outbound,
            finished: false,
        }
    }

    /// The id the service gave this stream.
    pub fn stream_id(&self) -> u64 {
        self.stream
    }

    /// The next item, events and bytes alike: `Some(Ok(item))`, `Some(Err(error))` when the service ended the stream with an error, and `None` once the stream is over, including after the connection is lost.
    pub async fn next_item(&mut self) -> Option<Result<StreamItem, ErrorBody>> {
        let item = self.items.recv().await;
        if !matches!(item, Some(Ok(_))) {
            self.finished = true;
        }
        // The service does not know this subscriber gave up, so the stream is cancelled here.
        if matches!(&item, Some(Err(error)) if error.code == codes::STREAM_OVERFLOW) {
            self.send_cancel();
        }
        item
    }

    /// The next event. Bytes items are skipped; read them with `next_item`. Otherwise as `next_item`.
    pub async fn next(&mut self) -> Option<Result<Value, ErrorBody>> {
        loop {
            match self.next_item().await? {
                Ok(StreamItem::Event(value)) => return Some(Ok(value)),
                Ok(StreamItem::Bytes(_)) => {}
                Err(error) => return Some(Err(error)),
            }
        }
    }

    /// Like `next`, with each event decoded as `T`.
    pub async fn next_as<T: DeserializeOwned>(&mut self) -> Option<Result<T, ClientError>> {
        match self.next().await? {
            Ok(value) => Some(
                serde_json::from_value(value)
                    .map_err(|error| ClientError::Protocol(error.to_string())),
            ),
            Err(error) => Some(Err(ClientError::Remote(error))),
        }
    }

    /// Stops the stream. Same as dropping it.
    pub fn cancel(self) {}

    /// Asks the service to stop the stream. A full outbound queue is waited for on a task, so the cancel is not lost.
    fn send_cancel(&self) {
        let cancel = Message::Cancel {
            stream: self.stream,
        };
        match self.outbound.try_send(cancel) {
            Ok(()) | Err(mpsc::error::TrySendError::Closed(_)) => {}
            Err(mpsc::error::TrySendError::Full(cancel)) => {
                if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                    let outbound = self.outbound.clone();
                    runtime.spawn(async move {
                        let _ = outbound.send(cancel).await;
                    });
                }
            }
        }
    }
}

impl fmt::Debug for Subscription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Subscription")
            .field("stream", &self.stream)
            .finish_non_exhaustive()
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if !self.finished {
            self.send_cancel();
        }
    }
}
