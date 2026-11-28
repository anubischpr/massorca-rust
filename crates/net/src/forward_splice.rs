//! One tunnelled TCP connection of a port forward (decision 0131), the same on the host and on the client: a socket on
//! one side, and the encoded [`Chunk`]s of its `host.forward_connect` stream on the other.
//!
//! Each direction may have at most `window` data chunks unacknowledged; a chunk is acknowledged once it is written to the
//! receiving socket, so a slow reader slows the sender instead of filling a queue, and a peer past the window ends the
//! tunnel. Acknowledgements are handled apart from socket writes, so two sides that both send bulk data never wait on
//! each other. End of data is half-closed: the receiver shuts its socket's write side and keeps reading.

use std::future::Future;
use std::io;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use massorca_protocol::methods::forwards::Chunk;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{Semaphore, mpsc};
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
    // Data chunks the peer sent that are not yet written and acknowledged; the window bounds them.
    let unacknowledged = AtomicU32::new(0);
    let progress = Progress {
        start: Instant::now(),
        last_activity_ms: AtomicU64::new(0),
    };
    let touch = || progress.touch();
    // The peer's data waits here for the socket. One slot more than the window holds end of data.
    let (writes, mut queued) = mpsc::channel::<Write>(limits.window as usize + 1);

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
                return Ok(());
            };
            send(&outgoing, Chunk::Data(bytes)).await?;
            sent.fetch_add(read as u64, Ordering::Relaxed);
        }
    };

    // The peer's chunks. Acknowledgements are taken here at once, apart from socket writes, so a socket that does not
    // read never holds back this side's credit. It runs until the peer's stream ends.
    let inbound = async {
        let writes = writes;
        let mut peer_eof = false;
        while let Some(payload) = incoming.recv().await {
            touch();
            match Chunk::decode(&payload) {
                Ok(Chunk::Data(bytes)) if bytes.len() <= limits.max_chunk => {
                    if peer_eof {
                        return SpliceEnd::Protocol("data after end of data".to_owned());
                    }
                    if unacknowledged.fetch_add(1, Ordering::Relaxed) >= limits.window {
                        return SpliceEnd::Protocol(
                            "more chunks than the window allows".to_owned(),
                        );
                    }
                    if writes.try_send(Write::Data(payload)).is_err() {
                        return SpliceEnd::Protocol(
                            "more chunks than the window allows".to_owned(),
                        );
                    }
                }
                Ok(Chunk::Data(bytes)) => {
                    return SpliceEnd::Protocol(format!("a chunk of {} bytes", bytes.len()));
                }
                Ok(Chunk::Eof) if !peer_eof => {
                    peer_eof = true;
                    if writes.try_send(Write::Eof).is_err() {
                        return SpliceEnd::Protocol("end of data past the window".to_owned());
                    }
                }
                Ok(Chunk::Eof) => return SpliceEnd::Protocol("end of data twice".to_owned()),
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
        // Dropping the queue's sender lets the socket writer finish what is queued, then stop.
        drop(writes);
        SpliceEnd::PeerGone
    };

    // Peer to socket: writes queued data in order, acknowledging each chunk once written.
    let socket_writer = async {
        while let Some(write) = queued.recv().await {
            match write {
                Write::Data(payload) => {
                    let bytes = payload.get(1..).unwrap_or_default();
                    writer.write_all(bytes).await.map_err(SpliceEnd::Io)?;
                    received.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                    unacknowledged.fetch_sub(1, Ordering::Relaxed);
                    touch();
                    // A peer that already finished both ways needs no acknowledgement; the queued data is still written.
                    let _ = send(&outgoing, Chunk::Ack(1)).await;
                }
                Write::Eof => {
                    // A socket whose peer already closed fails to shut down; the end of data still holds.
                    let _ = writer.shutdown().await;
                    return Ok(Written::Eof);
                }
            }
        }
        Ok(Written::Drained)
    };

    let end = supervise(
        upstream,
        inbound,
        socket_writer,
        stop,
        limits.idle,
        &progress,
    )
    .await;
    SpliceReport {
        end,
        sent: sent.load(Ordering::Relaxed),
        received: received.load(Ordering::Relaxed),
    }
}

/// A queued write to the socket.
enum Write {
    /// A whole data payload, tag byte included.
    Data(Vec<u8>),
    Eof,
}

/// How the socket writer stopped without failing.
enum Written {
    /// It wrote end of data.
    Eof,
    /// The peer's stream ended and everything queued was written.
    Drained,
}

/// When a tunnel last moved.
struct Progress {
    start: Instant,
    last_activity_ms: AtomicU64,
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

/// Runs the three parts until the tunnel closes (both sides sent end of data), one fails, the peer goes, `stop` resolves
/// or it idles. A peer that goes after both ends of data closes cleanly once its queued data is written.
async fn supervise(
    upstream: impl Future<Output = Result<(), SpliceEnd>>,
    inbound: impl Future<Output = SpliceEnd>,
    socket_writer: impl Future<Output = Result<Written, SpliceEnd>>,
    stop: impl Future<Output = ()>,
    idle: Duration,
    progress: &Progress,
) -> SpliceEnd {
    tokio::pin!(upstream, inbound, socket_writer, stop);
    let (mut upstream_done, mut peer_gone, mut written, mut peer_eof) =
        (false, false, false, false);
    let check_every = (idle / 4).max(Duration::from_millis(10));
    let mut ticks = tokio::time::interval(check_every);
    loop {
        if upstream_done && peer_eof {
            return SpliceEnd::Closed;
        }
        if peer_gone && written {
            return SpliceEnd::PeerGone;
        }
        tokio::select! {
            () = &mut stop => return SpliceEnd::Stopped,
            result = &mut upstream, if !upstream_done => match result {
                Ok(()) => upstream_done = true,
                Err(end) => return end,
            },
            end = &mut inbound, if !peer_gone => match end {
                SpliceEnd::PeerGone => peer_gone = true,
                end => return end,
            },
            result = &mut socket_writer, if !written => match result {
                Ok(Written::Eof) => (written, peer_eof) = (true, true),
                Ok(Written::Drained) => written = true,
                Err(end) => return end,
            },
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

    const BULK: SpliceLimits = SpliceLimits {
        window: 4,
        max_chunk: 1024,
        idle: Duration::from_secs(30),
    };

    /// Bulk data both ways at once, against a target that writes all of its bulk before it reads anything. While its
    /// socket write waits, acknowledgements must still flow, or the target's own data runs out of credit and both sides
    /// hang until the idle timeout.
    #[tokio::test]
    async fn bulk_both_ways_with_a_target_that_only_writes_for_a_while_does_not_deadlock() {
        let (client_app, client_socket) = duplex(1024);
        let (target_app, target_socket) = duplex(1024);
        let (client_out, target_in) = mpsc::channel(16);
        let (target_out, client_in) = mpsc::channel(16);
        let client = tokio::spawn(splice(
            client_socket,
            client_out,
            client_in,
            std::future::pending(),
            BULK,
        ));
        let target = tokio::spawn(splice(
            target_socket,
            target_out,
            target_in,
            std::future::pending(),
            BULK,
        ));
        let bulk: Vec<u8> = (0..512 * 1024).map(|at| (at % 251) as u8).collect();
        // The client writes and reads at the same time.
        let client_side = async {
            let (mut read, mut write) = tokio::io::split(client_app);
            let writing = async {
                write.write_all(&bulk).await.expect("the client writes");
                write.shutdown().await.expect("the client ends its data");
            };
            let mut got = Vec::new();
            let ((), read) = tokio::join!(writing, read.read_to_end(&mut got));
            read.expect("the client reads");
            got
        };
        // The target writes everything first, and only then reads.
        let target_side = async {
            let (mut read, mut write) = tokio::io::split(target_app);
            write.write_all(&bulk).await.expect("the target writes");
            write.shutdown().await.expect("the target ends its data");
            let mut got = Vec::new();
            read.read_to_end(&mut got).await.expect("the target reads");
            got
        };
        let both = async { tokio::join!(client_side, target_side) };
        let (at_client, at_target) = tokio::time::timeout(Duration::from_secs(20), both)
            .await
            .expect("bulk both ways finishes without a deadlock");
        let ends = (client.await.unwrap(), target.await.unwrap());
        let first = |got: &[u8]| got.iter().zip(&bulk).position(|(a, b)| a != b);
        assert!(
            at_client == bulk && at_target == bulk,
            "the bulk came back changed: client {} bytes, first difference {:?}; target {} bytes, first difference {:?}",
            at_client.len(),
            first(&at_client),
            at_target.len(),
            first(&at_target)
        );
        let (client, target) = ends;
        assert!(matches!(client.end, SpliceEnd::Closed), "{client:?}");
        assert!(matches!(target.end, SpliceEnd::Closed), "{target:?}");
    }

    /// A peer that sends more unacknowledged chunks than the window allows breaks the rules and ends the tunnel, instead of
    /// queueing without limit behind a socket that does not read.
    #[tokio::test]
    async fn a_peer_past_the_window_is_a_protocol_error() {
        let (_app, socket) = duplex(1);
        let (out, _peer_in) = mpsc::channel(64);
        let (peer_out, incoming) = mpsc::channel(64);
        let task = tokio::spawn(splice(
            socket,
            out,
            incoming,
            std::future::pending(),
            LIMITS,
        ));
        for _ in 0..=LIMITS.window {
            peer_out.send(Chunk::Data(b"abcd").encode()).await.unwrap();
        }
        let report = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("the tunnel ends")
            .unwrap();
        assert!(matches!(report.end, SpliceEnd::Protocol(_)), "{report:?}");
    }
}
