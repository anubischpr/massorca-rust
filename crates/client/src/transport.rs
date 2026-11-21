//! Reader and writer tasks for the two halves of a local socket. Only the reader reads, so no frame read is ever cancelled.

use std::sync::Arc;

use interprocess::local_socket::tokio::Stream;
use massorca_protocol::{MAX_FRAME_LEN_LOCAL, Message, read_message, write_message};
use tokio::sync::mpsc;

/// Reads frames until the service closes the connection or a read fails; the messages go to `inbound`.
pub(crate) async fn read_frames(stream: Arc<Stream>, inbound: mpsc::Sender<Message>) {
    let mut recv: &Stream = &stream;
    loop {
        match read_message(&mut recv, MAX_FRAME_LEN_LOCAL).await {
            Ok(Some(message)) => {
                if inbound.send(message).await.is_err() {
                    return;
                }
            }
            Ok(None) | Err(_) => return,
        }
    }
}

/// Writes queued requests in order until every sender is gone or a write fails.
pub(crate) async fn write_frames(stream: Arc<Stream>, mut outbound: mpsc::Receiver<Message>) {
    let mut send: &Stream = &stream;
    while let Some(message) = outbound.recv().await {
        if write_message(&mut send, &message, MAX_FRAME_LEN_LOCAL)
            .await
            .is_err()
        {
            return;
        }
    }
}
