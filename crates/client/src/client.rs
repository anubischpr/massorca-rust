//! A connected client: calls and subscriptions over one connection, and the welcome the service answered with.

use std::fmt;
use std::sync::Arc;

use interprocess::local_socket::tokio::Stream;
use massorca_profile::Profile;
use massorca_protocol::Message;
use massorca_protocol::methods::events::Subscribed;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::{mpsc, oneshot};

use crate::connect;
use crate::error::ClientError;
use crate::handshake::Welcome;
use crate::options::ConnectOptions;
use crate::pump::{self, Pending, Shared};
use crate::subscription::Subscription;
use crate::transport;

const INBOUND_CAPACITY: usize = 64;

/// A connection to a service. Clones share it, and it ends when the last clone is dropped.
#[derive(Clone)]
pub struct Client {
    shared: Arc<Shared>,
    welcome: Arc<Welcome>,
    outbound: mpsc::UnboundedSender<Message>,
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
        self.send_request(method, params, Pending::Call(reply))?;
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
        let (items, received) = mpsc::unbounded_channel();
        self.send_request(method, params, Pending::Subscribe { reply, items })?;
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

    fn send_request<P>(&self, method: &str, params: &P, pending: Pending) -> Result<(), ClientError>
    where
        P: Serialize + ?Sized,
    {
        let params = serde_json::to_value(params)
            .map_err(|error| ClientError::Protocol(error.to_string()))?;
        let id = self.shared.next_id();
        self.shared.register(id, pending)?;
        let request = Message::Request {
            id,
            method: method.to_owned(),
            params,
        };
        if self.outbound.send(request).is_err() {
            self.shared.forget(id);
            return Err(ClientError::Disconnected);
        }
        Ok(())
    }

    /// Starts the tasks of an established connection: the frame reader and writer, and the pump that routes their frames.
    pub(crate) fn open(stream: Stream, welcome: Welcome) -> Self {
        let stream = Arc::new(stream);
        let (inbound_tx, inbound) = mpsc::channel(INBOUND_CAPACITY);
        let (outbound, outbound_rx) = mpsc::unbounded_channel();
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
