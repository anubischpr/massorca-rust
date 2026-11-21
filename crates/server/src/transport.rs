//! Reader and writer tasks for the two halves of a local socket. Only the reader reads, so no frame read is ever cancelled.

use std::sync::Arc;

use interprocess::local_socket::tokio::Stream;
use massorca_protocol::{MAX_FRAME_LEN_LOCAL, Message, read_message, write_message};
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

/// Writes queued messages in order until the queue closes or a write fails.
pub(crate) async fn write_frames(stream: Arc<Stream>, mut outbound: mpsc::Receiver<Message>) {
    let mut send: &Stream = &stream;
    while let Some(message) = outbound.recv().await {
        if let Err(error) = write_message(&mut send, &message).await {
            tracing::debug!(%error, "connection write ended");
            return;
        }
    }
}
