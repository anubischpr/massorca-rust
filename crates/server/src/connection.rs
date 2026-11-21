//! One client connection: the handshake, then requests and streams until the peer leaves or the service stops.

use std::sync::Arc;

use interprocess::local_socket::tokio::Stream;
use massorca_protocol::{ErrorBody, Message, codes};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use crate::abort_guard::AbortOnDrop;
use crate::context::AppState;
use crate::dispatch;
use crate::handshake;
use crate::session::Session;
use crate::shutdown::ShutdownSignal;
use crate::streams::StreamTable;
use crate::transport;

const INBOUND_CAPACITY: usize = 64;
const OUTBOUND_CAPACITY: usize = 256;
/// Requests one connection may have in flight. At the limit it reads no further frames, so the peer waits on the socket.
const MAX_REQUESTS_IN_FLIGHT: usize = 64;

/// Serves one accepted connection until the peer leaves or the service stops.
pub(crate) async fn serve(stream: Stream, state: Arc<AppState>, mut signal: ShutdownSignal) {
    let stream = Arc::new(stream);
    let (inbound_tx, mut inbound) = mpsc::channel(INBOUND_CAPACITY);
    let (outbound, outbound_rx) = mpsc::channel(OUTBOUND_CAPACITY);

    // Aborting on drop keeps a cancelled connection from leaving its socket open.
    let reader = tokio::spawn(transport::read_frames(Arc::clone(&stream), inbound_tx));
    let _reader = AbortOnDrop::new(reader.abort_handle());
    let writer = tokio::spawn(transport::write_frames(Arc::clone(&stream), outbound_rx));
    let _writer = AbortOnDrop::new(writer.abort_handle());
    // The socket closes once both tasks have dropped their clones.
    drop(stream);

    if let Some(negotiated) = handshake::accept(&mut inbound, &outbound, &state).await {
        let session = Arc::new(Session {
            state,
            connection_id: negotiated.connection_id,
            capabilities: negotiated.capabilities,
            outbound: outbound.clone(),
            streams: StreamTable::default(),
        });
        run_session(&session, &mut inbound, &mut signal).await;
    }

    // Closing the last sender lets the writer flush what is queued (a refusal, the final replies) before the socket closes.
    drop(outbound);
    let _ = writer.await;
}

/// Routes requests until the peer leaves or shutdown is requested, then lets in-flight requests finish and ends open streams.
async fn run_session(
    session: &Arc<Session>,
    inbound: &mut mpsc::Receiver<Message>,
    signal: &mut ShutdownSignal,
) {
    let mut requests = JoinSet::new();
    let shutting_down = loop {
        tokio::select! {
            biased;
            () = signal.wait() => break true,
            Some(_) = requests.join_next(), if !requests.is_empty() => {}
            message = inbound.recv(), if requests.len() < MAX_REQUESTS_IN_FLIGHT => match message {
                Some(message) => route(message, session, &mut requests),
                None => break false,
            },
        }
    };

    // The accept loop bounds this wait when the service is stopping.
    while requests.join_next().await.is_some() {}
    for stream in session.streams.close_all() {
        if shutting_down {
            let error =
                ErrorBody::new(codes::SERVICE_SHUTTING_DOWN, "the service is shutting down");
            let _ = session
                .outbound
                .send(Message::End {
                    stream,
                    error: Some(error),
                })
                .await;
        }
    }
}

fn route(message: Message, session: &Arc<Session>, requests: &mut JoinSet<()>) {
    match message {
        Message::Request { id, method, params } => {
            requests.spawn(dispatch::run(Arc::clone(session), id, method, params));
        }
        Message::Cancel { stream } => session.streams.release(stream),
        // A repeated hello, frames only the service sends, and unknown types carry nothing to act on.
        _ => {}
    }
}
