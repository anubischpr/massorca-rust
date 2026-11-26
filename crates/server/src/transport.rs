//! Frame transports. A connection reads and writes frames through two tasks and two queues, so the code above them does
//! not know which transport it runs on (decision 0040). Only the reader reads, so no frame read is ever cancelled.

use std::sync::Arc;

use interprocess::local_socket::tokio::Stream;
use massorca_profile::LocalStream;
use massorca_protocol::{
    ErrorBody, Frame, FrameError, MAX_FRAME_LEN_LOCAL, Message, codes, read_frame, write_frame,
};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const INBOUND_CAPACITY: usize = 64;
const OUTBOUND_CAPACITY: usize = 256;

/// The two queues of one connection and the tasks that move frames between them and the transport.
pub(crate) struct FrameIo {
    pub(crate) inbound: mpsc::Receiver<Frame>,
    pub(crate) outbound: mpsc::Sender<Frame>,
    pub(crate) reader: JoinHandle<()>,
    pub(crate) writer: JoinHandle<()>,
    /// The largest frame body either side may send on this transport.
    pub(crate) max_frame_len: u32,
}

/// The queues a transport's tasks use: the reader sends into `inbound`, the writer drains `outbound`.
pub(crate) struct FrameQueues {
    pub(crate) inbound: mpsc::Sender<Frame>,
    pub(crate) outbound: mpsc::Receiver<Frame>,
}

/// Creates the queues, lets `start` spawn the transport's reader and writer on them, and returns the connection's ends.
pub(crate) fn spawn_with(
    max_frame_len: u32,
    start: impl FnOnce(FrameQueues) -> (JoinHandle<()>, JoinHandle<()>),
) -> FrameIo {
    let (inbound_tx, inbound) = mpsc::channel(INBOUND_CAPACITY);
    let (outbound, outbound_rx) = mpsc::channel(OUTBOUND_CAPACITY);
    let (reader, writer) = start(FrameQueues {
        inbound: inbound_tx,
        outbound: outbound_rx,
    });
    FrameIo {
        inbound,
        outbound,
        reader,
        writer,
        max_frame_len,
    }
}

/// The transport of the local socket: `u32` length-prefixed frames (decision 0002).
pub(crate) fn spawn_local(stream: Stream) -> FrameIo {
    spawn_with(MAX_FRAME_LEN_LOCAL, |queues| {
        let stream = Arc::new(LocalStream::new(stream));
        let reader = tokio::spawn(read_frames(Arc::clone(&stream), queues.inbound));
        let writer = tokio::spawn(write_frames(stream, queues.outbound));
        (reader, writer)
    })
}

/// Reads frames until the peer closes or a read fails; the frames go to `inbound`.
async fn read_frames(stream: Arc<LocalStream>, inbound: mpsc::Sender<Frame>) {
    let mut recv: &Stream = stream.get();
    loop {
        match read_frame(&mut recv, MAX_FRAME_LEN_LOCAL).await {
            Ok(Some(frame)) => {
                if inbound.send(frame).await.is_err() {
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

/// Writes queued frames in order until the queue closes or a write fails. A reply too large for one frame is
/// replaced by a `response.too_large` error for its request; nothing of the oversized frame was written.
async fn write_frames(stream: Arc<LocalStream>, mut outbound: mpsc::Receiver<Frame>) {
    let mut send: &Stream = stream.get();
    while let Some(frame) = outbound.recv().await {
        let written = match write_frame(&mut send, &frame, MAX_FRAME_LEN_LOCAL).await {
            Err(FrameError::TooLarge(len)) => {
                match too_large_reply(&frame, len, MAX_FRAME_LEN_LOCAL) {
                    Some(reply) => {
                        write_frame(&mut send, &Frame::Message(reply), MAX_FRAME_LEN_LOCAL).await
                    }
                    None => Err(FrameError::TooLarge(len)),
                }
            }
            other => other,
        };
        if let Err(error) = written {
            tracing::debug!(%error, "connection write ended");
            return;
        }
    }
}

/// The error that stands in for a reply that cannot be framed; `None` for any other frame.
pub(crate) fn too_large_reply(frame: &Frame, len: u32, max_len: u32) -> Option<Message> {
    let Frame::Message(Message::Response { id, .. }) = frame else {
        return None;
    };
    let error = ErrorBody::new(
        codes::RESPONSE_TOO_LARGE,
        format!("the reply is {len} bytes, more than one frame may hold ({max_len})"),
    );
    Some(Message::response_err(*id, error))
}
