//! The pump of a connection: routes replies and events to the calls and subscriptions waiting for them, and fails everything once the connection ends.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use massorca_protocol::{ErrorBody, Message};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::error::ClientError;

/// The outcome of a call: its result, or the error the service sent.
pub(crate) type Reply = Result<Value, ErrorBody>;

/// One item of a subscription: an event, or the error that ended the stream.
pub(crate) type Item = Result<Value, ErrorBody>;

/// A request waiting for its answer.
pub(crate) enum Pending {
    Call(oneshot::Sender<Reply>),
    /// The answer names the stream. Its items are routed from the moment the answer is read, so none can arrive unrouted.
    Subscribe {
        reply: oneshot::Sender<Reply>,
        items: mpsc::UnboundedSender<Item>,
    },
}

#[derive(Default)]
struct Routes {
    closed: bool,
    pending: HashMap<u64, Pending>,
    streams: HashMap<u64, mpsc::UnboundedSender<Item>>,
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

    /// Hands one incoming message to whoever waits for it. Frames nobody waits for are dropped.
    pub(crate) fn route(&self, message: Message) {
        match message {
            Message::Response { id, result, error } => self.answer(id, reply(result, error)),
            Message::Event { stream, data } => self.deliver(stream, data),
            Message::End { stream, error } => self.end(stream, error),
            _ => {}
        }
    }

    /// Ends the connection for every waiter: pending calls fail as disconnected, and open streams end.
    pub(crate) fn close(&self) {
        let mut routes = lock(&self.routes);
        routes.closed = true;
        routes.pending.clear();
        routes.streams.clear();
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

    fn deliver(&self, stream: u64, data: Value) {
        let mut routes = lock(&self.routes);
        let gone = match routes.streams.get(&stream) {
            Some(items) => items.send(Ok(data)).is_err(),
            None => false,
        };
        if gone {
            routes.streams.remove(&stream);
        }
    }

    fn end(&self, stream: u64, error: Option<ErrorBody>) {
        let ended = lock(&self.routes).streams.remove(&stream);
        // Dropping the sender ends the subscription; an error is delivered just before that.
        if let (Some(items), Some(error)) = (ended, error) {
            let _ = items.send(Err(error));
        }
    }
}

/// Routes frames until the connection ends, then closes the shared state and stops both socket tasks.
pub(crate) async fn run(
    shared: Arc<Shared>,
    mut inbound: mpsc::Receiver<Message>,
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
    shared.close();
    reader.abort();
    writer.abort();
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
