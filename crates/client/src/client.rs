//! A connected client: calls and subscriptions over one connection, and the welcome the service answered with.

use std::fmt;
use std::io;
use std::sync::Arc;

#[cfg(feature = "local")]
use massorca_profile::Profile;
use massorca_protocol::methods::events::Subscribed;
use massorca_protocol::{Frame, Message, fits_frame, fits_stream_bytes};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tokio::task::AbortHandle;

#[cfg(feature = "local")]
use crate::connect;
use crate::error::ClientError;
use crate::handshake::Welcome;
#[cfg(feature = "local")]
use crate::options::ConnectOptions;
use crate::pump::{self, Pending, Shared};
use crate::subscription::Subscription;
use crate::transport::FrameIo;

/// Items a subscriber may leave unread before its stream ends with `stream.overflow`.
const SUBSCRIPTION_CAPACITY: usize = 256;

/// A connection to a service. Clones share it, and it ends when the last clone is dropped or one of them calls
/// `close`.
#[derive(Clone)]
pub struct Client {
    shared: Arc<Shared>,
    welcome: Arc<Welcome>,
    outbound: mpsc::Sender<Frame>,
    /// The socket reader and writer; stopping them ends the connection for every clone.
    sockets: Arc<[AbortHandle; 2]>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("welcome", &self.welcome)
            .finish_non_exhaustive()
    }
}

impl Client {
    #[cfg(feature = "local")]
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

    /// Ends the connection for every clone at once: pending calls fail as disconnected and open streams end as lost.
    pub fn close(&self) {
        for socket in self.sockets.iter() {
            socket.abort();
        }
    }

    /// Resolves once the connection has ended: the service closed it, or it was lost. Cancel-safe.
    pub async fn closed(&self) {
        self.shared.closed().await;
    }

    /// Whether the connection has ended, without waiting. See [`Client::closed`].
    pub fn is_closed(&self) -> bool {
        self.shared.is_closed()
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
        let max_frame_len = self.welcome.max_frame_len;
        if !fits_frame(&request, max_frame_len) {
            return Err(ClientError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("the request is larger than one frame may hold ({max_frame_len} bytes)"),
            )));
        }
        self.shared.register(id, pending)?;
        if self.outbound.send(Frame::Message(request)).await.is_err() {
            self.shared.forget(id);
            return Err(ClientError::Disconnected);
        }
        Ok(())
    }

    /// Sends stream bytes on a stream the service reads them on (decision 0131). Waits for room in the outbound queue.
    pub(crate) async fn send_stream_bytes(
        &self,
        stream: u64,
        payload: Vec<u8>,
    ) -> Result<(), ClientError> {
        let max_frame_len = self.welcome.max_frame_len;
        if !fits_stream_bytes(&payload, max_frame_len) {
            return Err(ClientError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("stream bytes are larger than one frame may hold ({max_frame_len} bytes)"),
            )));
        }
        self.outbound
            .send(Frame::StreamBytes { stream, payload })
            .await
            .map_err(|_| ClientError::Disconnected)
    }

    /// Starts the tasks of an established connection: the frame reader and writer, and the pump that routes their frames.
    pub(crate) fn open(io: FrameIo, welcome: Welcome) -> Self {
        let FrameIo {
            inbound,
            outbound,
            reader,
            writer,
            ..
        } = io;
        let shared = Arc::new(Shared::default());
        let sockets = Arc::new([reader.abort_handle(), writer.abort_handle()]);
        tokio::spawn(pump::run(Arc::clone(&shared), inbound, reader, writer));
        Self {
            shared,
            welcome: Arc::new(welcome),
            outbound,
            sockets,
        }
    }
}

/// Encodes call params. A value that cannot be encoded is reported before anything is sent.
fn encode_params<P: Serialize + ?Sized>(params: &P) -> Result<Value, ClientError> {
    serde_json::to_value(params).map_err(|error| ClientError::Protocol(error.to_string()))
}
