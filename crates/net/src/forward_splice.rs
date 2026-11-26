//! One tunnelled TCP connection of a port forward (decision 0131), the same on the host and on the client: a socket on
//! one side, and the encoded [`Chunk`]s of its `host.forward_connect` stream on the other.
//!
//! Each direction may have at most `window` data chunks unacknowledged; a chunk is acknowledged once it is written to the
//! receiving socket, so a slow reader slows the sender instead of filling a queue. End of data is half-closed: the
//! receiver shuts its socket's write side and keeps reading.

use std::future::Future;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use massorca_protocol::methods::forwards::Chunk;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{Notify, Semaphore, mpsc};
use tokio::time::Instant;

/// The limits of one tunnel.
#[derive(Debug, Clone, Copy)]
pub struct SpliceLimits {
    pub window: u32,
    pub max_chunk: usize,
    pub idle: Duration,
}

/// Why a tunnel ended.
#[derive(Debug)]
pub enum SpliceEnd {
    /// Both sides sent end of data.
    Closed,
    /// The peer's stream ended before both sides finished.
    PeerGone,
    /// The forward was stopped.
    Stopped,
    /// Nothing moved either way for `idle`.
    Idle,
    /// The peer broke the chunk rules.
    Protocol(String),
    /// The socket failed.
    Io(io::Error),
}

/// How a tunnel ended, and the bytes it carried.
#[derive(Debug)]
pub struct SpliceReport {
    pub end: SpliceEnd,
    /// Bytes read from the socket and sent to the peer.
    pub sent: u64,
    /// Bytes received from the peer and written to the socket.
    pub received: u64,
}

/// Carries one connection until both sides end, the peer goes, `stop` resolves, it idles, or it fails. `outgoing` takes
/// encoded chunks for the peer; `incoming` gives the peer's.
pub async fn splice<S>(
    socket: S,
    outgoing: mpsc::Sender<Vec<u8>>,
    mut incoming: mpsc::Receiver<Vec<u8>>,
    stop: impl Future<Output = ()>,
    limits: SpliceLimits,
) -> SpliceReport
where
    S: AsyncRead + AsyncWrite,
{
    let (mut reader, mut writer) = tokio::io::split(socket);
    let credits = Semaphore::new(limits.window as usize);
    let sent = AtomicU64::new(0);
    let received = AtomicU64::new(0);
    let progress = Progress {
        start: Instant::now(),
        last_activity_ms: AtomicU64::new(0),
        peer_eof: AtomicBool::new(false),
        sent_eof: AtomicBool::new(false),
        peer_eof_seen: Notify::new(),
    };
    let touch = || progress.touch();

    // Socket to peer: one chunk per credit, so at most `window` chunks wait for an acknowledgement.
    let upstream = async {
        let mut buffer = vec![0; limits.max_chunk];
        loop {
            let Ok(permit) = credits.acquire().await else {
                return Err(SpliceEnd::PeerGone);
            };
            permit.forget();
            let read = reader.read(&mut buffer).await.map_err(SpliceEnd::Io)?;
            touch();
            let Some(bytes) = buffer.get(..read).filter(|bytes| !bytes.is_empty()) else {
                send(&outgoing, Chunk::Eof).await?;
                progress.sent_eof.store(true, Ordering::Relaxed);
                return Ok(());
            };
            send(&outgoing, Chunk::Data(bytes)).await?;
            sent.fetch_add(read as u64, Ordering::Relaxed);
        }
    };

    // Peer to socket. It runs until the peer's stream ends, since acknowledgements keep coming after end of data.
    let downstream = async {
        while let Some(payload) = incoming.recv().await {
            touch();
            match Chunk::decode(&payload) {
                Ok(Chunk::Data(bytes)) if bytes.len() <= limits.max_chunk => {
                    if progress.peer_eof.load(Ordering::Relaxed) {
                        return SpliceEnd::Protocol("data after end of data".to_owned());
                    }
                    if let Err(error) = writer.write_all(bytes).await {
                        return SpliceEnd::Io(error);
                    }
                    received.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                    if let Err(end) = send(&outgoing, Chunk::Ack(1)).await {
                        return end;
                    }
                }
                Ok(Chunk::Data(bytes)) => {
                    return SpliceEnd::Protocol(format!("a chunk of {} bytes", bytes.len()));
                }
                Ok(Chunk::Eof) => {
                    // A socket whose peer already closed fails to shut down; the end of data still holds.
                    let _ = writer.shutdown().await;
                    progress.peer_eof.store(true, Ordering::Relaxed);
                    progress.peer_eof_seen.notify_one();
                }
                Ok(Chunk::Ack(count)) => {
                    let after = credits.available_permits().saturating_add(count as usize);
                    if after > limits.window as usize {
                        return SpliceEnd::Protocol(
                            "more acknowledgements than chunks sent".to_owned(),
                        );
                    }
                    credits.add_permits(count as usize);
                }
                Err(error) => return SpliceEnd::Protocol(error.to_string()),
            }
        }
        SpliceEnd::PeerGone
    };

    let end = supervise(upstream, downstream, stop, limits.idle, &progress).await;
    SpliceReport {
        end,
        sent: sent.load(Ordering::Relaxed),
        received: received.load(Ordering::Relaxed),
    }
}

/// What both directions of a tunnel share: when it last moved, and whether the peer sent end of data.
struct Progress {
    start: Instant,
    last_activity_ms: AtomicU64,
    peer_eof: AtomicBool,
    sent_eof: AtomicBool,
    peer_eof_seen: Notify,
}

impl Progress {
    fn touch(&self) {
        let elapsed = u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.last_activity_ms.store(elapsed, Ordering::Relaxed);
    }

    fn quiet_for(&self) -> Duration {
        let last = Duration::from_millis(self.last_activity_ms.load(Ordering::Relaxed));
        self.start.elapsed().saturating_sub(last)
    }
}

async fn supervise(
    upstream: impl Future<Output = Result<(), SpliceEnd>>,
    downstream: impl Future<Output = SpliceEnd>,
    stop: impl Future<Output = ()>,
    idle: Duration,
    progress: &Progress,
) -> SpliceEnd {
    tokio::pin!(upstream, downstream, stop);
    let mut upstream_done = false;
    let check_every = (idle / 4).max(Duration::from_millis(10));
    let mut ticks = tokio::time::interval(check_every);
    loop {
        if upstream_done && progress.peer_eof.load(Ordering::Relaxed) {
            return SpliceEnd::Closed;
        }
        tokio::select! {
            () = &mut stop => return SpliceEnd::Stopped,
            result = &mut upstream, if !upstream_done => match result {
                Ok(()) => upstream_done = true,
                Err(end) => return end,
            },
            // A peer that finished both ways may leave before the last acknowledgements: that is a clean close.
            end = &mut downstream => {
                let both_ended = progress.sent_eof.load(Ordering::Relaxed)
                    && progress.peer_eof.load(Ordering::Relaxed);
                return if both_ended && matches!(end, SpliceEnd::PeerGone) {
                    SpliceEnd::Closed
                } else {
                    end
                };
            }
            () = progress.peer_eof_seen.notified() => {}
            _ = ticks.tick() => {
                if progress.quiet_for() >= idle {
                    return SpliceEnd::Idle;
                }
            }
        }
    }
}

async fn send(outgoing: &mpsc::Sender<Vec<u8>>, chunk: Chunk<'_>) -> Result<(), SpliceEnd> {
    outgoing
        .send(chunk.encode())
        .await
        .map_err(|_| SpliceEnd::PeerGone)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use massorca_protocol::methods::forwards::Chunk;
    use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};
    use tokio::sync::mpsc;

    use super::{SpliceEnd, SpliceLimits, splice};

    const LIMITS: SpliceLimits = SpliceLimits {
        window: 2,
        max_chunk: 4,
        idle: Duration::from_secs(30),
    };

    /// Two splices joined back to back carry bytes both ways and close on both ends of data.
    #[tokio::test]
    async fn two_splices_carry_bytes_both_ways_and_close_together() {
        let (left_app, left_socket) = duplex(64);
        let (right_app, right_socket) = duplex(64);
        let (left_out, right_in) = mpsc::channel(8);
        let (right_out, left_in) = mpsc::channel(8);
        let left = tokio::spawn(splice(
            left_socket,
            left_out,
            left_in,
            std::future::pending(),
            LIMITS,
        ));
        let right = tokio::spawn(splice(
            right_socket,
            right_out,
            right_in,
            std::future::pending(),
            LIMITS,
        ));
        let (mut left_read, mut left_write) = tokio::io::split(left_app);
        let (mut right_read, mut right_write) = tokio::io::split(right_app);
        let message = b"a message longer than the window of chunks".to_vec();
        left_write.write_all(&message).await.unwrap();
        left_write.shutdown().await.unwrap();
        let mut got = Vec::new();
        right_read.read_to_end(&mut got).await.unwrap();
        assert_eq!(got, message);
        right_write.write_all(b"reply").await.unwrap();
        right_write.shutdown().await.unwrap();
        let mut reply = Vec::new();
        left_read.read_to_end(&mut reply).await.unwrap();
        assert_eq!(reply, b"reply");
        let left = left.await.unwrap();
        let right = right.await.unwrap();
        assert!(matches!(left.end, SpliceEnd::Closed), "{left:?}");
        assert!(matches!(right.end, SpliceEnd::Closed), "{right:?}");
        assert_eq!(left.sent, message.len() as u64);
        assert_eq!(right.received, message.len() as u64);
    }

    /// Without acknowledgements the sender stops after `window` chunks, and an acknowledgement it never earned is a
    /// protocol error.
    #[tokio::test]
    async fn the_window_holds_back_unacknowledged_chunks() {
        let (app, socket) = duplex(64);
        let (out, mut peer_in) = mpsc::channel(8);
        let (peer_out, incoming) = mpsc::channel(8);
        let task = tokio::spawn(splice(
            socket,
            out,
            incoming,
            std::future::pending(),
            LIMITS,
        ));
        let (_read, mut write) = tokio::io::split(app);
        write.write_all(b"aaaabbbbcccc").await.unwrap();
        let mut chunks = Vec::new();
        for _ in 0..2 {
            chunks.push(peer_in.recv().await.unwrap());
        }
        assert!(
            tokio::time::timeout(Duration::from_millis(200), peer_in.recv())
                .await
                .is_err(),
            "a third chunk went out unacknowledged"
        );
        peer_out.send(Chunk::Ack(1).encode()).await.unwrap();
        assert_eq!(
            Chunk::decode(&peer_in.recv().await.unwrap()),
            Ok(Chunk::Data(b"cccc"))
        );
        peer_out.send(Chunk::Ack(5).encode()).await.unwrap();
        let report = task.await.unwrap();
        assert!(matches!(report.end, SpliceEnd::Protocol(_)), "{report:?}");
    }

    #[tokio::test]
    async fn the_peer_leaving_stop_and_idle_end_the_tunnel() {
        let (_app, socket) = duplex(64);
        let (out, _peer_in) = mpsc::channel(8);
        let (peer_out, incoming) = mpsc::channel::<Vec<u8>>(8);
        drop(peer_out);
        let report = splice(socket, out, incoming, std::future::pending(), LIMITS).await;
        assert!(matches!(report.end, SpliceEnd::PeerGone), "{report:?}");

        let (_app, socket) = duplex(64);
        let (out, _peer_in) = mpsc::channel(8);
        let (_peer_out, incoming) = mpsc::channel::<Vec<u8>>(8);
        let report = splice(socket, out, incoming, async {}, LIMITS).await;
        assert!(matches!(report.end, SpliceEnd::Stopped), "{report:?}");

        let (_app, socket) = duplex(64);
        let (out, _peer_in) = mpsc::channel(8);
        let (_peer_out, incoming) = mpsc::channel::<Vec<u8>>(8);
        let idle = SpliceLimits {
            idle: Duration::from_millis(100),
            ..LIMITS
        };
        let report = splice(socket, out, incoming, std::future::pending(), idle).await;
        assert!(matches!(report.end, SpliceEnd::Idle), "{report:?}");
    }
}
