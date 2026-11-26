//! One client connection: the handshake, then requests and streams until the peer leaves or the service stops.

use std::sync::Arc;
use std::time::Instant;

use massorca_protocol::{ErrorBody, Frame, Message, codes};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::abort_guard::AbortOnDrop;
use crate::context::AppState;
use crate::dispatch;
use crate::handshake;
use crate::network::{LiveGuard, Pending, revoked_error};
use crate::session::Session;
use crate::shutdown::ShutdownSignal;
use crate::streams::StreamTable;
use crate::transport::FrameIo;
use massorca_net::Pin;
use std::net::SocketAddr;

/// Requests one connection may have in flight. At the limit it reads no further frames, so the peer waits on the socket.
const MAX_REQUESTS_IN_FLIGHT: usize = 64;

/// Where a connection came from, which decides how its principal is found.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Origin {
    /// The owner-only local socket: the owner of the profile.
    Local,
    /// The network: a device identified by its TLS client certificate.
    Network { client_pin: Pin, peer: SocketAddr },
}

/// Serves one accepted connection until the peer leaves or the service stops.
pub(crate) async fn serve(
    io: FrameIo,
    origin: Origin,
    state: Arc<AppState>,
    mut signal: ShutdownSignal,
    mut pre_auth: Option<Pending>,
) {
    let FrameIo {
        mut inbound,
        outbound,
        reader,
        mut writer,
        max_frame_len,
    } = io;
    // Aborting on drop keeps a cancelled connection from leaving its transport open.
    let _reader = AbortOnDrop::new(reader.abort_handle());
    let _writer = AbortOnDrop::new(writer.abort_handle());

    let mut shutdown_seen = None;
    let mut connection_id = None;
    let serving = async {
        if let Some(negotiated) =
            handshake::accept(&mut inbound, &outbound, &state, max_frame_len, &origin).await
        {
            if let Some(pending) = pre_auth.take() {
                pending.authenticated();
            }
            // Revoking the device closes this connection at once (decision 0041).
            connection_id = Some(negotiated.connection_id.clone());
            let mut live = negotiated.live;
            let session = Arc::new(Session {
                state,
                connection_id: negotiated.connection_id,
                capabilities: negotiated.capabilities,
                client_capabilities: Arc::new(negotiated.client_capabilities),
                principal: negotiated.principal,
                outbound: outbound.clone(),
                max_frame_len,
                streams: StreamTable::default(),
            });
            shutdown_seen =
                run_session(&session, &mut inbound, &mut signal, live.as_mut()).await;
        }
    };
    let writer_ended = tokio::select! {
        () = serving => false,
        // Replies that can no longer be written leave nothing to serve, so the session ends with its writer.
        _ = &mut writer => true,
    };

    // Closing the last sender lets the writer flush what is queued (a refusal, the final replies) before the socket closes.
    drop(outbound);
    if !writer_ended {
        let _ = writer.await;
    }
    // Dropping an unopened gate ends the reader, and with it the writer; a refusal must be flushed first.
    drop(pre_auth);
    // Timing only, no payloads: tells a connection that never returned from one that returned late.
    tracing::debug!(
        connection = connection_id.as_deref().unwrap_or("none"),
        writer_ended,
        since_shutdown = ?shutdown_seen.map(|seen: Instant| seen.elapsed()),
        "connection served"
    );
}

/// Why a session stopped reading requests.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Closing {
    PeerLeft,
    ShuttingDown,
    Revoked,
}

/// Routes requests until the peer leaves, shutdown is requested or the device is revoked, then ends open streams.
/// A shutdown lets in-flight requests finish; the other two cancel them.
async fn run_session(
    session: &Arc<Session>,
    inbound: &mut mpsc::Receiver<Frame>,
    signal: &mut ShutdownSignal,
    mut live: Option<&mut LiveGuard>,
) -> Option<Instant> {
    let mut requests = JoinSet::new();
    let closing = loop {
        tokio::select! {
            biased;
            () = signal.wait() => break Closing::ShuttingDown,
            () = revoked(live.as_deref_mut()) => {
                tracing::info!(connection = %session.connection_id, "closing the connection of a revoked device");
                break Closing::Revoked;
            }
            Some(_) = requests.join_next(), if !requests.is_empty() => {}
            frame = inbound.recv(), if requests.len() < MAX_REQUESTS_IN_FLIGHT => match frame {
                Some(frame) => route(frame, session, &mut requests),
                None => break Closing::PeerLeft,
            },
        }
    };

    let shutdown_seen = (closing == Closing::ShuttingDown).then(Instant::now);
    // Each open stream is told why it ends, so a client does not mistake it for a finished stream.
    let reason = match closing {
        Closing::PeerLeft => None,
        Closing::ShuttingDown => Some(ErrorBody::new(
            codes::SERVICE_SHUTTING_DOWN,
            "the service is shutting down",
        )),
        Closing::Revoked => Some(revoked_error()),
    };
    if closing == Closing::ShuttingDown {
        // Subscribers learn at once, before the in-flight requests drain.
        end_streams(session, reason.as_ref()).await;
    } else {
        // Nobody is left to read the replies, or the device may no longer have them, so the requests are cancelled.
        requests.abort_all();
    }
    // The accept loop bounds this wait when the service is stopping.
    while requests.join_next().await.is_some() {}
    // Also ends streams that requests opened while draining.
    end_streams(session, reason.as_ref()).await;
    shutdown_seen
}

async fn end_streams(session: &Session, reason: Option<&ErrorBody>) {
    for stream in session.streams.close_all() {
        if let Some(error) = reason {
            let _ = session
                .outbound
                .send(Frame::Message(Message::End {
                    stream,
                    error: Some(error.clone()),
                }))
                .await;
        }
    }
}

fn route(frame: Frame, session: &Arc<Session>, requests: &mut JoinSet<()>) {
    // No client sends stream bytes yet, so they are dropped like the other frames nothing acts on.
    let Frame::Message(message) = frame else {
        return;
    };
    match message {
        Message::Request { id, method, params } => {
            requests.spawn(dispatch::run(Arc::clone(session), id, method, params));
        }
        Message::Cancel { stream } => session.streams.release(stream),
        // A repeated hello, frames only the service sends, and unknown types carry nothing to act on.
        _ => {}
    }
}

/// Resolves once the connection's device is revoked; never for a connection without a device.
async fn revoked(live: Option<&mut LiveGuard>) {
    match live {
        Some(live) => {
            let _ = live.revoked.wait_for(|revoked| *revoked).await;
        }
        None => std::future::pending().await,
    }
}
