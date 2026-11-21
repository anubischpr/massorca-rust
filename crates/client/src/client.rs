//! A connected client: calls and subscriptions over one connection, and the welcome the service answered with.

use std::fmt;
use std::io;
use std::sync::Arc;

use interprocess::local_socket::tokio::Stream;
use massorca_profile::Profile;
use massorca_protocol::methods::events::Subscribed;
use massorca_protocol::{MAX_FRAME_LEN_LOCAL, Message, fits_frame};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use crate::connect;
use crate::error::ClientError;
use crate::handshake::Welcome;
use crate::options::ConnectOptions;
use crate::pump::{self, Pending, Shared};
use crate::subscription::Subscription;
use crate::transport;

const INBOUND_CAPACITY: usize = 64;
/// Requests and cancels queued for the writer. A caller waits for room rather than queueing without limit.
const OUTBOUND_CAPACITY: usize = 256;
/// Items a subscriber may leave unread before its stream ends with `stream.overflow`.
const SUBSCRIPTION_CAPACITY: usize = 256;

/// A connection to a service. Clones share it, and it ends when the last clone is dropped.
#[derive(Clone)]
pub struct Client {
    shared: Arc<Shared>,
    welcome: Arc<Welcome>,
    outbound: mpsc::Sender<Message>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("welcome", &self.welcome)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Connects to the service of `profile`. With autostart enabled and nothing listening, the service is started first.
    pub async fn connect(profile: &Profile, options: ConnectOptions) -> Result<Self, ClientError> {
        connect::connect(profile, options).await
    }

    /// What the service offered when the connection was made.
    pub fn welcome(&self) -> &Welcome {
        &self.welcome
    }

    /// Whether the service offered `name` in its welcome.
    pub fn has_capability(&self, name: &str) -> bool {
        self.welcome
            .capabilities
            .iter()
            .any(|offered| offered == name)
    }

    /// Calls a method and decodes its result. A failure the service reports is `ClientError::Remote`.
    pub async fn call<P, R>(&self, method: &str, params: &P) -> Result<R, ClientError>
    where
        P: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let (reply, answer) = oneshot::channel();
        self.send_request(method, encode_params(params)?, Pending::Call(reply))
            .await?;
        let value = answer
            .await
            .map_err(|_| ClientError::Disconnected)?
            .map_err(ClientError::Remote)?;
        serde_json::from_value(value).map_err(|error| ClientError::Protocol(error.to_string()))
    }

    /// Calls a method that opens a stream, and returns the subscription to its items.
    pub async fn subscribe<P>(&self, method: &str, params: &P) -> Result<Subscription, ClientError>
    where
        P: Serialize + ?Sized,
    {
        let (reply, answer) = oneshot::channel();
        // One slot beyond the limit is kept for the error that ends a subscriber which falls behind.
        let (items, received) = mpsc::channel(SUBSCRIPTION_CAPACITY + 1);
        self.send_request(
            method,
            encode_params(params)?,
            Pending::Subscribe { reply, items },
        )
        .await?;
        let value = answer
            .await
            .map_err(|_| ClientError::Disconnected)?
            .map_err(ClientError::Remote)?;
        let opened: Subscribed = serde_json::from_value(value)
            .map_err(|error| ClientError::Protocol(error.to_string()))?;
        Ok(Subscription::new(
            opened.stream,
            received,
            self.outbound.clone(),
        ))
    }

    async fn send_request(
        &self,
        method: &str,
        params: Value,
        pending: Pending,
    ) -> Result<(), ClientError> {
        let id = self.shared.next_id();
        let request = Message::Request {
            id,
            method: method.to_owned(),
            params,
        };
        // Refused before it is registered: the service would close the connection on a frame it cannot read.
        if !fits_frame(&request, MAX_FRAME_LEN_LOCAL) {
            return Err(ClientError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "the request is larger than one frame may hold ({MAX_FRAME_LEN_LOCAL} bytes)"
                ),
            )));
        }
        self.shared.register(id, pending)?;
        if self.outbound.send(request).await.is_err() {
            self.shared.forget(id);
            return Err(ClientError::Disconnected);
        }
        Ok(())
    }

    /// Starts the tasks of an established connection: the frame reader and writer, and the pump that routes their frames.
    pub(crate) fn open(stream: Stream, welcome: Welcome) -> Self {
        let stream = Arc::new(stream);
        let (inbound_tx, inbound) = mpsc::channel(INBOUND_CAPACITY);
        let (outbound, outbound_rx) = mpsc::channel(OUTBOUND_CAPACITY);
        let reader = tokio::spawn(transport::read_frames(Arc::clone(&stream), inbound_tx));
        let writer = tokio::spawn(transport::write_frames(stream, outbound_rx));
        let shared = Arc::new(Shared::default());
        tokio::spawn(pump::run(Arc::clone(&shared), inbound, reader, writer));
        Self {
            shared,
            welcome: Arc::new(welcome),
            outbound,
        }
    }
}

/// Encodes call params. A value that cannot be encoded is reported before anything is sent.
fn encode_params<P: Serialize + ?Sized>(params: &P) -> Result<Value, ClientError> {
    serde_json::to_value(params).map_err(|error| ClientError::Protocol(error.to_string()))
}
