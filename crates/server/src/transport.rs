//! Reader and writer tasks for the two halves of a local socket. Only the reader reads, so no frame read is ever cancelled.

use std::sync::Arc;

use interprocess::local_socket::tokio::Stream;
use massorca_protocol::{
    ErrorBody, FrameError, MAX_FRAME_LEN_LOCAL, Message, codes, read_message, write_message,
};
use tokio::sync::mpsc;

/// Reads frames until the peer closes or a read fails; the messages go to `inbound`.
pub(crate) async fn read_frames(stream: Arc<Stream>, inbound: mpsc::Sender<Message>) {
    let mut recv: &Stream = &stream;
    loop {
        match read_message(&mut recv, MAX_FRAME_LEN_LOCAL).await {
            Ok(Some(message)) => {
                if inbound.send(message).await.is_err() {
                    return;
                }
            }
            Ok(None) => return,
            Err(error) => {
                tracing::debug!(%error, "connection read ended");
                return;
            }
        }
    }
}

/// Writes queued messages in order until the queue closes or a write fails. A reply too large for one frame is
/// replaced by a `response.too_large` error for its request; nothing of the oversized frame was written.
pub(crate) async fn write_frames(stream: Arc<Stream>, mut outbound: mpsc::Receiver<Message>) {
    let mut send: &Stream = &stream;
    while let Some(message) = outbound.recv().await {
        let written = match write_message(&mut send, &message, MAX_FRAME_LEN_LOCAL).await {
            Err(FrameError::TooLarge(len)) => match too_large_reply(&message, len) {
                Some(reply) => write_message(&mut send, &reply, MAX_FRAME_LEN_LOCAL).await,
                None => Err(FrameError::TooLarge(len)),
            },
            other => other,
        };
        if let Err(error) = written {
            tracing::debug!(%error, "connection write ended");
            return;
        }
    }
}

/// The error that stands in for a reply that cannot be framed; `None` for any other message.
fn too_large_reply(message: &Message, len: u32) -> Option<Message> {
    let Message::Response { id, .. } = message else {
        return None;
    };
    let error = ErrorBody::new(
        codes::RESPONSE_TOO_LARGE,
        format!("the reply is {len} bytes, more than one frame may hold ({MAX_FRAME_LEN_LOCAL})"),
    );
    Some(Message::response_err(*id, error))
}
