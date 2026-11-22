use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use massorca_protocol::codes::RESPONSE_TOO_LARGE;
use massorca_protocol::{ErrorBody, Frame, Message, decode_frame_body, encode_frame_body, fits};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::{MissedTickBehavior, interval_at, timeout};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// How long the writer waits for its Close frame to go out before dropping the connection.
const CLOSE_GRACE: Duration = Duration::from_secs(1);

/// Keep-alive of decision 0040: ping every `interval`, close after `silence` without any frame, ping or pong.
#[derive(Debug, Clone, Copy)]
pub struct Keepalive {
    pub interval: Duration,
    pub silence: Duration,
}

impl Default for Keepalive {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(20),
            silence: Duration::from_secs(60),
        }
    }
}

/// Holds a connection's reader to one small frame until the peer has authenticated (decision 0040), so an
/// unauthenticated peer cannot fill the inbound queue. Made by [`pre_auth_gate`].
#[derive(Debug)]
pub struct PreAuthGate {
    opened: watch::Receiver<bool>,
    max_len: u32,
}

/// Opens the matching [`PreAuthGate`]. Dropping it unopened ends the reader once it would wait.
#[derive(Debug)]
pub struct PreAuthOpener(watch::Sender<bool>);

impl PreAuthOpener {
    pub fn open(&self) {
        let _ = self.0.send(true);
    }
}

/// A closed gate that lets one frame of at most `max_len` bytes through before it is opened.
pub fn pre_auth_gate(max_len: u32) -> (PreAuthOpener, PreAuthGate) {
    let (sender, opened) = watch::channel(false);
    (PreAuthOpener(sender), PreAuthGate { opened, max_len })
}

/// [`spawn_frame_tasks`] whose reader stays behind `gate` until it opens.
pub fn spawn_gated_frame_tasks<S, T>(
    ws: WebSocketStream<S>,
    max_len: u32,
    keepalive: Keepalive,
    gate: PreAuthGate,
    inbound: mpsc::Sender<Frame>,
    outbound: mpsc::Receiver<T>,
) -> (JoinHandle<()>, JoinHandle<()>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    T: Into<Frame> + Send + 'static,
{
    spawn_tasks(ws, max_len, keepalive, Some(gate), inbound, outbound)
}

/// Moves frames between `ws` and two queues, one binary WebSocket message per frame body.
///
/// The reader ends (dropping `inbound`) on text, an undecodable frame, a message above `max_len`, close, error, or
/// `silence` with nothing received; ping and pong only refresh the silence timer. The writer sends `outbound` in
/// order plus a ping every `interval`, and ends when the queue closes (sending Close), a write fails, or the reader
/// ends. A frame above `max_len` is never written: a response becomes `response.too_large` for its id, anything
/// else ends the writer.
pub fn spawn_frame_tasks<S, T>(
    ws: WebSocketStream<S>,
    max_len: u32,
    keepalive: Keepalive,
    inbound: mpsc::Sender<Frame>,
    outbound: mpsc::Receiver<T>,
) -> (JoinHandle<()>, JoinHandle<()>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    T: Into<Frame> + Send + 'static,
{
    spawn_tasks(ws, max_len, keepalive, None, inbound, outbound)
}

fn spawn_tasks<S, T>(
    ws: WebSocketStream<S>,
    max_len: u32,
    keepalive: Keepalive,
    gate: Option<PreAuthGate>,
    inbound: mpsc::Sender<Frame>,
    outbound: mpsc::Receiver<T>,
) -> (JoinHandle<()>, JoinHandle<()>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    T: Into<Frame> + Send + 'static,
{
    let (sink, stream) = ws.split();
    // Dropped when the reader ends, so the writer lets go of its half and the connection closes.
    let (reader_alive, reader_gone) = oneshot::channel::<()>();
    let reader = tokio::spawn(async move {
        read_frames(stream, max_len, keepalive.silence, gate, inbound).await;
        drop(reader_alive);
    });
    let writer = tokio::spawn(write_frames(
        sink,
        max_len,
        keepalive.interval,
        outbound,
        reader_gone,
    ));
    (reader, writer)
}

async fn read_frames<St>(
    mut stream: St,
    max_len: u32,
    silence: Duration,
    mut gate: Option<PreAuthGate>,
    inbound: mpsc::Sender<Frame>,
) where
    St: futures_util::Stream<Item = Result<WsMessage, tokio_tungstenite::tungstenite::Error>>
        + Unpin,
{
    let max = usize::try_from(max_len).unwrap_or(usize::MAX);
    let mut received = 0_usize;
    loop {
        if let Some(closed) = gate.as_mut() {
            if received > 0 && closed.opened.wait_for(|open| *open).await.is_err() {
                return;
            }
            if *closed.opened.borrow() {
                gate = None;
            }
        }
        let limit = gate.as_ref().map_or(max, |closed| {
            usize::try_from(closed.max_len).unwrap_or(usize::MAX)
        });
        let message = match timeout(silence, stream.next()).await {
            Err(_) => {
                tracing::debug!("network peer silent; closing");
                return;
            }
            Ok(None) => return,
            Ok(Some(Err(error))) => {
                tracing::debug!(%error, "network read failed");
                return;
            }
            Ok(Some(Ok(message))) => message,
        };
        match message {
            WsMessage::Binary(body) => {
                if body.len() > limit {
                    tracing::debug!(len = body.len(), "network frame above the limit");
                    return;
                }
                let Ok(frame) = decode_frame_body(&body) else {
                    tracing::debug!("network frame does not decode");
                    return;
                };
                if inbound.send(frame).await.is_err() {
                    return;
                }
                received += 1;
            }
            // tungstenite answers pings itself on the next read or write.
            WsMessage::Ping(_) | WsMessage::Pong(_) => {}
            WsMessage::Text(_) | WsMessage::Close(_) | WsMessage::Frame(_) => return,
        }
    }
}

async fn write_frames<Si, T>(
    mut sink: Si,
    max_len: u32,
    interval: Duration,
    mut outbound: mpsc::Receiver<T>,
    mut reader_gone: oneshot::Receiver<()>,
) where
    Si: futures_util::Sink<WsMessage, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
    T: Into<Frame>,
{
    let mut ticker = interval_at(tokio::time::Instant::now() + interval, interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        let message = tokio::select! {
            biased;
            _ = &mut reader_gone => return,
            item = outbound.recv() => match item {
                Some(item) => match bounded(item.into(), max_len) {
                    Some(frame) => WsMessage::binary(encode_frame_body(&frame)),
                    None => break,
                },
                None => break,
            },
            _ = ticker.tick() => WsMessage::Ping(Default::default()),
        };
        // A stalled write must not outlive the reader's verdict on the peer.
        let sent = tokio::select! {
            biased;
            _ = &mut reader_gone => return,
            sent = sink.send(message) => sent,
        };
        if let Err(error) = sent {
            tracing::debug!(%error, "network write failed");
            return;
        }
    }
    let _ = timeout(CLOSE_GRACE, sink.close()).await;
}

/// `frame` if it fits, a `response.too_large` error in place of an oversized response, or `None` to end the writer.
fn bounded(frame: Frame, max_len: u32) -> Option<Frame> {
    if fits(&frame, max_len) {
        return Some(frame);
    }
    match frame {
        Frame::Message(Message::Response { id, .. }) => {
            tracing::debug!(id, "network response above the frame limit");
            Some(Frame::Message(Message::response_err(
                id,
                ErrorBody::new(
                    RESPONSE_TOO_LARGE,
                    "the response exceeds the connection's frame limit",
                ),
            )))
        }
        _ => {
            tracing::debug!("network frame above the limit; closing");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio_tungstenite::tungstenite::protocol::Role;

    use super::*;

    const LIMIT: u32 = 1024 * 1024;
    const PRE_AUTH: u32 = 256;

    /// A server reader behind a gate, and the raw client side of the same WebSocket.
    async fn gated() -> (
        PreAuthOpener,
        mpsc::Receiver<Frame>,
        JoinHandle<()>,
        WebSocketStream<tokio::io::DuplexStream>,
        mpsc::Sender<Frame>,
    ) {
        let (server_io, client_io) = tokio::io::duplex(64 * 1024);
        let server = WebSocketStream::from_raw_socket(server_io, Role::Server, None).await;
        let client = WebSocketStream::from_raw_socket(client_io, Role::Client, None).await;
        let (opener, gate) = pre_auth_gate(PRE_AUTH);
        let (inbound_tx, inbound) = mpsc::channel(64);
        let (outbound, outbound_rx) = mpsc::channel::<Frame>(8);
        let (reader, _writer) = spawn_gated_frame_tasks(
            server,
            LIMIT,
            Keepalive::default(),
            gate,
            inbound_tx,
            outbound_rx,
        );
        (opener, inbound, reader, client, outbound)
    }

    fn cancel(stream: u64) -> WsMessage {
        WsMessage::binary(encode_frame_body(&Frame::Message(Message::Cancel {
            stream,
        })))
    }

    #[tokio::test]
    async fn before_authentication_the_reader_takes_one_frame_then_waits() {
        let (opener, mut inbound, _reader, mut client, _outbound) = gated().await;
        for stream in 1..=3 {
            client.send(cancel(stream)).await.unwrap();
        }
        let first = timeout(Duration::from_secs(5), inbound.recv())
            .await
            .unwrap();
        assert!(matches!(
            first,
            Some(Frame::Message(Message::Cancel { stream: 1 }))
        ));
        assert!(
            timeout(Duration::from_millis(200), inbound.recv())
                .await
                .is_err(),
            "nothing more is read before the gate opens"
        );
        opener.open();
        for expected in 2..=3 {
            let next = timeout(Duration::from_secs(5), inbound.recv())
                .await
                .unwrap();
            assert!(
                matches!(next, Some(Frame::Message(Message::Cancel { stream })) if stream == expected)
            );
        }
    }

    #[tokio::test]
    async fn before_authentication_a_large_frame_closes_the_connection() {
        let (_opener, mut inbound, reader, mut client, _outbound) = gated().await;
        let payload = vec![0_u8; usize::try_from(PRE_AUTH).unwrap() * 2];
        let body = encode_frame_body(&Frame::StreamBytes { stream: 1, payload });
        client.send(WsMessage::binary(body)).await.unwrap();
        assert!(
            timeout(Duration::from_secs(5), inbound.recv())
                .await
                .unwrap()
                .is_none()
        );
        timeout(Duration::from_secs(5), reader)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn an_unopened_gate_ends_the_reader_when_dropped() {
        let (opener, mut inbound, reader, mut client, _outbound) = gated().await;
        client.send(cancel(1)).await.unwrap();
        client.send(cancel(2)).await.unwrap();
        assert!(inbound.recv().await.is_some());
        drop(opener);
        timeout(Duration::from_secs(5), reader)
            .await
            .unwrap()
            .unwrap();
    }
}
