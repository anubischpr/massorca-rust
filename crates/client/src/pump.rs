//! The pump of a connection: routes replies and events to the calls and subscriptions waiting for them, and fails everything once the connection ends.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use massorca_protocol::{ErrorBody, Frame, Message, codes};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::error::ClientError;
use crate::subscription::StreamItem;

/// The outcome of a call: its result, or the error the service sent.
pub(crate) type Reply = Result<Value, ErrorBody>;

/// One item of a subscription's queue.
pub(crate) enum Item {
    /// An event or bytes.
    Next(StreamItem),
    /// The error the service ended the stream with.
    Failed(ErrorBody),
    /// The connection ended while the stream was open.
    Lost,
}

/// A request waiting for its answer.
pub(crate) enum Pending {
    Call(oneshot::Sender<Reply>),
    /// The answer names the stream. Its items are routed from the moment the answer is read, so none can arrive unrouted.
    Subscribe {
        reply: oneshot::Sender<Reply>,
        items: mpsc::Sender<Item>,
    },
}

#[derive(Default)]
struct Routes {
    closed: bool,
    pending: HashMap<u64, Pending>,
    streams: HashMap<u64, mpsc::Sender<Item>>,
}

/// State shared by the client handles and the pump of one connection.
#[derive(Default)]
pub(crate) struct Shared {
    last_id: AtomicU64,
    routes: Mutex<Routes>,
}

impl Shared {
    pub(crate) fn next_id(&self) -> u64 {
        self.last_id.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Records a request as waiting. Refused once the connection has ended.
    pub(crate) fn register(&self, id: u64, pending: Pending) -> Result<(), ClientError> {
        let mut routes = lock(&self.routes);
        if routes.closed {
            return Err(ClientError::Disconnected);
        }
        routes.pending.insert(id, pending);
        Ok(())
    }

    pub(crate) fn forget(&self, id: u64) {
        lock(&self.routes).pending.remove(&id);
    }

    /// Hands one incoming frame to whoever waits for it. Frames nobody waits for are dropped.
    pub(crate) fn route(&self, frame: Frame) {
        match frame {
            Frame::Message(Message::Response { id, result, error }) => {
                self.answer(id, reply(result, error));
            }
            Frame::Message(Message::Event { stream, data }) => {
                self.deliver(stream, StreamItem::Event(data));
            }
            Frame::Message(Message::End { stream, error }) => self.end(stream, error),
            Frame::StreamBytes { stream, payload } => {
                self.deliver(stream, StreamItem::Bytes(payload));
            }
            _ => {}
        }
    }

    /// Ends the connection for every waiter: pending calls fail as disconnected, and open streams end as lost.
    pub(crate) fn close(&self) {
        let mut routes = lock(&self.routes);
        routes.closed = true;
        routes.pending.clear();
        // The slot `deliver` keeps back is free while a stream is routed, so the marker always fits.
        for (_, items) in routes.streams.drain() {
            let _ = items.try_send(Item::Lost);
        }
    }

    fn answer(&self, id: u64, reply: Reply) {
        let mut routes = lock(&self.routes);
        match routes.pending.remove(&id) {
            Some(Pending::Call(sender)) => {
                let _ = sender.send(reply);
            }
            Some(Pending::Subscribe {
                reply: sender,
                items,
            }) => {
                if let Ok(value) = &reply
                    && let Some(stream) = value.get("stream").and_then(Value::as_u64)
                {
                    routes.streams.insert(stream, items);
                }
                let _ = sender.send(reply);
            }
            None => {}
        }
    }

    /// Queues one event or byte item for its subscriber. A subscriber whose queue is full is ended with `stream.overflow`.
    /// The last slot is kept back for that error, so it always fits, and items never take it.
    fn deliver(&self, stream: u64, item: StreamItem) {
        let mut routes = lock(&self.routes);
        let ended = match routes.streams.get(&stream) {
            Some(items) if items.capacity() > 1 => items.try_send(Item::Next(item)).is_err(),
            Some(items) => {
                let _ = items.try_send(Item::Failed(overflow()));
                true
            }
            None => false,
        };
        if ended {
            routes.streams.remove(&stream);
        }
    }

    fn end(&self, stream: u64, error: Option<ErrorBody>) {
        let ended = lock(&self.routes).streams.remove(&stream);
        // Dropping the sender ends the subscription; an error is delivered just before that, in the slot kept for it.
        if let (Some(items), Some(error)) = (ended, error) {
            let _ = items.try_send(Item::Failed(error));
        }
    }
}

/// Routes frames until the connection ends, then closes the shared state and stops both socket tasks.
pub(crate) async fn run(
    shared: Arc<Shared>,
    mut inbound: mpsc::Receiver<Frame>,
    reader: JoinHandle<()>,
    mut writer: JoinHandle<()>,
) {
    loop {
        tokio::select! {
            biased;
            _ = &mut writer => break,
            message = inbound.recv() => match message {
                Some(message) => shared.route(message),
                None => break,
            },
        }
    }
    // The writer ends once the reader has, so frames the reader queued before the close (such as the `end` that says
    // why a stream stopped) are routed before the streams are marked lost.
    while let Ok(message) = inbound.try_recv() {
        shared.route(message);
    }
    shared.close();
    reader.abort();
    writer.abort();
    // Timing only, no payloads: says when the client noticed the connection end.
    tracing::debug!("client pump ended");
}

fn overflow() -> ErrorBody {
    ErrorBody::new(
        codes::STREAM_OVERFLOW,
        "the subscriber fell behind, so the stream ended",
    )
}

fn reply(result: Option<Value>, error: Option<ErrorBody>) -> Reply {
    match (error, result) {
        (Some(error), _) => Err(error),
        (None, Some(value)) => Ok(value),
        (None, None) => Ok(Value::Null),
    }
}

fn lock(routes: &Mutex<Routes>) -> MutexGuard<'_, Routes> {
    routes.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reader queued a stream's `end` and the connection closed; the writer noticed first. The `end` still
    /// reaches the subscriber, so it learns why the stream stopped instead of seeing a lost connection.
    #[tokio::test]
    async fn an_end_queued_before_the_close_is_delivered_even_when_the_writer_stops_first() {
        let shared = Arc::new(Shared::default());
        let (reply, _answer) = oneshot::channel();
        let (items, mut received) = mpsc::channel(4);
        shared
            .register(1, Pending::Subscribe { reply, items })
            .expect("registered");
        shared.route(Frame::Message(Message::Response {
            id: 1,
            result: Some(serde_json::json!({ "stream": 7 })),
            error: None,
        }));

        let (inbound_tx, inbound) = mpsc::channel(4);
        let revoked = ErrorBody::new(codes::AUTH_DEVICE_REVOKED, "revoked");
        inbound_tx
            .send(Frame::Message(Message::End {
                stream: 7,
                error: Some(revoked.clone()),
            }))
            .await
            .expect("queued");
        let writer = tokio::spawn(async {});
        while !writer.is_finished() {
            tokio::task::yield_now().await;
        }
        let reader = tokio::spawn(std::future::pending::<()>());
        run(Arc::clone(&shared), inbound, reader, writer).await;

        match received.recv().await {
            Some(Item::Failed(error)) => assert_eq!(error, revoked),
            Some(Item::Lost) => panic!("the end was dropped and the stream read as lost"),
            Some(Item::Next(_)) | None => panic!("expected the end with its reason"),
        }
    }
}
