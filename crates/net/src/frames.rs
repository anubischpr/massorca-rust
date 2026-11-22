use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use massorca_protocol::codes::RESPONSE_TOO_LARGE;
use massorca_protocol::{ErrorBody, Frame, Message, decode_frame_body, encode_frame_body, fits};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot};
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
    let (sink, stream) = ws.split();
    // Dropped when the reader ends, so the writer lets go of its half and the connection closes.
    let (reader_alive, reader_gone) = oneshot::channel::<()>();
    let reader = tokio::spawn(async move {
        read_frames(stream, max_len, keepalive.silence, inbound).await;
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
    inbound: mpsc::Sender<Frame>,
) where
    St: futures_util::Stream<Item = Result<WsMessage, tokio_tungstenite::tungstenite::Error>>
        + Unpin,
{
    let max = usize::try_from(max_len).unwrap_or(usize::MAX);
    loop {
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
                if body.len() > max {
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
