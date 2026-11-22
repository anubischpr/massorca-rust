//! Frame transports. A connection reads and writes frames through two tasks and two queues, so the handshake and the
//! client above them do not know which transport they run on (decision 0040). Only the reader reads.

use std::sync::Arc;

use interprocess::local_socket::tokio::Stream;
use massorca_protocol::{Frame, MAX_FRAME_LEN_LOCAL, Message, read_frame, write_message};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const INBOUND_CAPACITY: usize = 64;
/// Requests and cancels queued for the writer. A caller waits for room rather than queueing without limit.
const OUTBOUND_CAPACITY: usize = 256;

/// The two queues of one connection and the tasks that move frames between them and the transport.
#[derive(Debug)]
pub struct FrameIo {
    pub(crate) inbound: mpsc::Receiver<Frame>,
    pub(crate) outbound: mpsc::Sender<Message>,
    pub(crate) reader: JoinHandle<()>,
    pub(crate) writer: JoinHandle<()>,
    /// The largest frame body this transport carries before the service names its own limit.
    pub(crate) max_frame_len: u32,
}

/// The queues a transport's tasks use: the reader sends into `inbound`, the writer drains `outbound`.
#[derive(Debug)]
pub struct FrameQueues {
    pub inbound: mpsc::Sender<Frame>,
    pub outbound: mpsc::Receiver<Message>,
}

impl FrameIo {
    /// Creates the queues and lets `start` spawn the transport's reader and writer on them.
    pub fn spawn_with(
        max_frame_len: u32,
        start: impl FnOnce(FrameQueues) -> (JoinHandle<()>, JoinHandle<()>),
    ) -> Self {
        let (inbound_tx, inbound) = mpsc::channel(INBOUND_CAPACITY);
        let (outbound, outbound_rx) = mpsc::channel(OUTBOUND_CAPACITY);
        let (reader, writer) = start(FrameQueues {
            inbound: inbound_tx,
            outbound: outbound_rx,
        });
        Self {
            inbound,
            outbound,
            reader,
            writer,
            max_frame_len,
        }
    }

    /// Stops both transport tasks, which closes the transport.
    pub(crate) fn abort(&self) {
        self.reader.abort();
        self.writer.abort();
    }
}

/// The transport of the local socket: `u32` length-prefixed frames (decision 0002).
pub(crate) fn spawn_local(stream: Stream) -> FrameIo {
    FrameIo::spawn_with(MAX_FRAME_LEN_LOCAL, |queues| {
        let stream = Arc::new(stream);
        let reader = tokio::spawn(read_frames(Arc::clone(&stream), queues.inbound));
        let writer = tokio::spawn(write_frames(stream, queues.outbound));
        (reader, writer)
    })
}

/// Reads frames until the service closes the connection or a read fails; the frames go to `inbound`.
async fn read_frames(stream: Arc<Stream>, inbound: mpsc::Sender<Frame>) {
    let mut recv: &Stream = &stream;
    loop {
        match read_frame(&mut recv, MAX_FRAME_LEN_LOCAL).await {
            Ok(Some(frame)) => {
                if inbound.send(frame).await.is_err() {
                    return;
                }
            }
            Ok(None) | Err(_) => return,
        }
    }
}

/// Writes queued requests in order until every sender is gone or a write fails.
async fn write_frames(stream: Arc<Stream>, mut outbound: mpsc::Receiver<Message>) {
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
