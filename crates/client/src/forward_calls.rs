//! Port forwards (decision 0131): opening a forward on the host, carrying local connections through it, and listing and
//! stopping forwards. Every call is refused before anything is sent when the host does not offer `port-forward/1`.

use std::time::Duration;

use massorca_net::{SpliceLimits, SpliceReport, splice};
use massorca_protocol::capabilities;
use massorca_protocol::methods::forwards::{
    self, ForwardConnectParams, ForwardEvent, ForwardInfo, ForwardList, ForwardParams,
    ForwardStopParams, ForwardStopped, IDLE_TIMEOUT_SECS, MAX_CHUNK, WINDOW_CHUNKS,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

use crate::client::Client;
use crate::error::ClientError;
use crate::subscription::{StreamItem, Subscription};

/// Chunks of one tunnel queued between the splice and the connection: a window of data and its acknowledgements fit.
const TUNNEL_QUEUE: usize = 2 * WINDOW_CHUNKS as usize + 8;

/// An open forward. It stays open on the host until this handle is dropped or the host stops it.
#[derive(Debug)]
pub struct ForwardHandle {
    info: ForwardInfo,
    subscription: Subscription,
}

/// How a forward ended.
#[derive(Debug)]
pub enum ForwardEnd {
    /// `host.forward_stop` stopped it on the host.
    Stopped(ForwardInfo),
    /// The connection to the host was lost.
    Lost,
    /// The host ended it with an error, or without saying why.
    Ended(Option<ClientError>),
}

impl ForwardHandle {
    /// The forward as the host opened it.
    pub fn info(&self) -> &ForwardInfo {
        &self.info
    }

    /// Waits until the forward ends.
    pub async fn ended(&mut self) -> ForwardEnd {
        loop {
            match self.subscription.next_as::<ForwardEvent>().await {
                Some(Ok(ForwardEvent::Stopped { forward })) => return ForwardEnd::Stopped(forward),
                Some(Ok(ForwardEvent::Opened { .. })) => {}
                Some(Err(error)) => return ForwardEnd::Ended(Some(error)),
                None if self.subscription.connection_lost() => return ForwardEnd::Lost,
                None => return ForwardEnd::Ended(None),
            }
        }
    }
}

impl Client {
    fn require_forwards(&self, method: &str) -> Result<(), ClientError> {
        for capability in [capabilities::PORT_FORWARD, capabilities::BINARY_FRAMES] {
            if !self.has_capability(capability) {
                return Err(ClientError::CapabilityMissing {
                    what: method.to_owned(),
                    capability: capability.to_owned(),
                });
            }
        }
        Ok(())
    }

    /// `host.forward`: opens a forward to a loopback port on the host.
    pub async fn forward_open(&self, params: &ForwardParams) -> Result<ForwardHandle, ClientError> {
        self.require_forwards(forwards::FORWARD)?;
        let mut subscription = self.subscribe(forwards::FORWARD, params).await?;
        match subscription.next_as::<ForwardEvent>().await {
            Some(Ok(ForwardEvent::Opened { forward })) => Ok(ForwardHandle {
                info: forward,
                subscription,
            }),
            Some(Err(error)) => Err(error),
            Some(Ok(ForwardEvent::Stopped { .. })) | None => Err(ClientError::Protocol(
                "the forward ended before it opened".to_owned(),
            )),
        }
    }

    /// `host.forwards`: the forwards open on the host.
    pub async fn forward_list(&self) -> Result<ForwardList, ClientError> {
        self.require_forwards(forwards::FORWARDS)?;
        self.call(forwards::FORWARDS, &()).await
    }

    /// `host.forward_stop`: stops a forward on the host and every connection it carries.
    pub async fn forward_stop(&self, forward: &str) -> Result<ForwardStopped, ClientError> {
        self.require_forwards(forwards::FORWARD_STOP)?;
        let params = ForwardStopParams {
            forward: forward.to_owned(),
        };
        self.call(forwards::FORWARD_STOP, &params).await
    }

    /// `host.forward_connect`: carries `socket` to the forward's port on the host until either side closes. Fails before
    /// carrying anything when the host cannot connect to its port.
    pub async fn forward_tunnel<S>(
        &self,
        forward: &str,
        socket: S,
    ) -> Result<SpliceReport, ClientError>
    where
        S: AsyncRead + AsyncWrite,
    {
        self.require_forwards(forwards::FORWARD_CONNECT)?;
        let params = ForwardConnectParams {
            forward: forward.to_owned(),
        };
        let subscription = self.subscribe(forwards::FORWARD_CONNECT, &params).await?;
        let stream = subscription.stream_id();
        let (incoming_sender, incoming) = mpsc::channel(TUNNEL_QUEUE);
        let (outgoing, mut outgoing_chunks) = mpsc::channel::<Vec<u8>>(TUNNEL_QUEUE);
        // The host's chunks, until the stream ends or the splice stops reading; the subscription drops with it, which
        // cancels the stream on the host.
        tokio::spawn(receive(subscription, incoming_sender));
        let sender = self.clone();
        tokio::spawn(async move {
            while let Some(chunk) = outgoing_chunks.recv().await {
                if sender.send_stream_bytes(stream, chunk).await.is_err() {
                    return;
                }
            }
        });
        let limits = SpliceLimits {
            window: WINDOW_CHUNKS,
            max_chunk: MAX_CHUNK,
            idle: Duration::from_secs(IDLE_TIMEOUT_SECS),
        };
        let closed = self.clone();
        Ok(splice(
            socket,
            outgoing,
            incoming,
            async move { closed.closed().await },
            limits,
        )
        .await)
    }
}

async fn receive(mut subscription: Subscription, incoming: mpsc::Sender<Vec<u8>>) {
    loop {
        let item = tokio::select! {
            item = subscription.next_item() => item,
            () = incoming.closed() => return,
        };
        match item {
            Some(Ok(StreamItem::Bytes(payload))) => {
                if incoming.send(payload).await.is_err() {
                    return;
                }
            }
            Some(Ok(StreamItem::Event(_))) => {}
            Some(Err(_)) | None => return,
        }
    }
}
