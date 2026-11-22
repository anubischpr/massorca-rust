//! One client connection: the handshake, then requests and streams until the peer leaves or the service stops.

use std::sync::Arc;

use massorca_protocol::{ErrorBody, Frame, Message, codes};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::abort_guard::AbortOnDrop;
use crate::context::AppState;
use crate::dispatch;
use crate::handshake;
use crate::session::Session;
use crate::shutdown::ShutdownSignal;
use crate::streams::StreamTable;
use crate::transport::FrameIo;

/// Requests one connection may have in flight. At the limit it reads no further frames, so the peer waits on the socket.
const MAX_REQUESTS_IN_FLIGHT: usize = 64;

/// Serves one accepted connection until the peer leaves or the service stops.
pub(crate) async fn serve(io: FrameIo, state: Arc<AppState>, mut signal: ShutdownSignal) {
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

    let serving = async {
        if let Some(negotiated) =
            handshake::accept(&mut inbound, &outbound, &state, max_frame_len).await
        {
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
            run_session(&session, &mut inbound, &mut signal).await;
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
}

/// Routes requests until the peer leaves or shutdown is requested, then lets in-flight requests finish and ends open streams.
async fn run_session(
    session: &Arc<Session>,
    inbound: &mut mpsc::Receiver<Frame>,
    signal: &mut ShutdownSignal,
) {
    let mut requests = JoinSet::new();
    let shutting_down = loop {
        tokio::select! {
            biased;
            () = signal.wait() => break true,
            Some(_) = requests.join_next(), if !requests.is_empty() => {}
            frame = inbound.recv(), if requests.len() < MAX_REQUESTS_IN_FLIGHT => match frame {
                Some(frame) => route(frame, session, &mut requests),
                None => break false,
            },
        }
    };

    if !shutting_down {
        // Nobody is left to read the replies, so the requests are cancelled rather than run to completion.
        requests.abort_all();
    }
    // The accept loop bounds this wait when the service is stopping.
    while requests.join_next().await.is_some() {}
    for stream in session.streams.close_all() {
        if shutting_down {
            let error =
                ErrorBody::new(codes::SERVICE_SHUTTING_DOWN, "the service is shutting down");
            let _ = session
                .outbound
                .send(Frame::Message(Message::End {
                    stream,
                    error: Some(error),
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
