//! A stream opened by a call. Dropping it asks the service to stop sending.

use std::fmt;

use massorca_protocol::{ErrorBody, Message};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::error::ClientError;
use crate::pump::Item;

/// The items of one stream. They queue until read; dropping the subscription cancels the stream on the service.
pub struct Subscription {
    stream: u64,
    items: mpsc::UnboundedReceiver<Item>,
    outbound: mpsc::UnboundedSender<Message>,
    finished: bool,
}

impl Subscription {
    pub(crate) fn new(
        stream: u64,
        items: mpsc::UnboundedReceiver<Item>,
        outbound: mpsc::UnboundedSender<Message>,
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

    /// The next item: `Some(Ok(value))` for an event, `Some(Err(error))` when the service ended the stream with an error, and `None` once the stream is over, including after the connection is lost.
    pub async fn next(&mut self) -> Option<Result<Value, ErrorBody>> {
        let item = self.items.recv().await;
        if !matches!(item, Some(Ok(_))) {
            self.finished = true;
        }
        item
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
            let _ = self.outbound.send(Message::Cancel {
                stream: self.stream,
            });
        }
    }
}
