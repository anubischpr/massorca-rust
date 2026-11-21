//! The server side of the handshake (ADR 0002): `hello` in, `welcome` out. Every refusal goes through `refuse`.

use std::collections::HashSet;
use std::time::Duration;

use massorca_core_store::new_id;
use massorca_protocol::{
    ErrorBody, Frame, MAX_FRAME_LEN_LOCAL, MIN_PROTOCOL_VERSION, Message, PROTOCOL_VERSION,
    ProtocolRange, codes, negotiate,
};
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::context::AppState;
use crate::permission::Principal;

/// How long a client may take to send its first frame.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);

/// The protocol range this build speaks, named in every refusal.
const SUPPORTED: ProtocolRange = ProtocolRange {
    min: MIN_PROTOCOL_VERSION,
    max: PROTOCOL_VERSION,
};

/// What a connection agreed in its handshake.
pub(crate) struct Negotiated {
    pub(crate) connection_id: String,
    /// The capabilities the service offered in `welcome`; a method of any other capability is refused.
    pub(crate) capabilities: HashSet<String>,
    /// The capabilities the client listed in `hello`. Recorded for handlers; no behaviour depends on them yet.
    pub(crate) client_capabilities: HashSet<String>,
    /// Who the connection acts as. A local connection acts as the owner of the profile.
    pub(crate) principal: Principal,
}

/// Answers the first frame. `None` closes the connection; a refusal, when one is due, is already queued.
pub(crate) async fn accept(
    inbound: &mut mpsc::Receiver<Frame>,
    outbound: &mpsc::Sender<Frame>,
    state: &AppState,
) -> Option<Negotiated> {
    let first = match timeout(HELLO_TIMEOUT, inbound.recv()).await {
        Ok(Some(first)) => first,
        // The peer left before saying anything; there is nobody to answer.
        Ok(None) => return None,
        Err(_elapsed) => {
            refuse(
                outbound,
                ErrorBody::new(codes::PROTOCOL_HANDSHAKE, "no hello within 10 seconds"),
            )
            .await;
            return None;
        }
    };
    let Frame::Message(Message::Hello {
        protocol,
        capabilities: offered,
        ..
    }) = first
    else {
        refuse(
            outbound,
            ErrorBody::new(codes::PROTOCOL_HANDSHAKE, "the first message must be hello"),
        )
        .await;
        return None;
    };
    let version = match negotiate(protocol, SUPPORTED) {
        Ok(version) => version,
        Err(error) => {
            refuse(outbound, error).await;
            return None;
        }
    };

    let capabilities = state.registries.capabilities.names();
    let connection_id = new_id();
    let welcome = Message::Welcome {
        protocol: version,
        server: state.server.clone(),
        capabilities: capabilities.clone(),
        connection_id: connection_id.clone(),
        max_frame_len: Some(MAX_FRAME_LEN_LOCAL),
    };
    outbound.send(Frame::Message(welcome)).await.ok()?;
    Some(Negotiated {
        connection_id,
        capabilities: capabilities.into_iter().collect(),
        client_capabilities: offered.into_iter().collect(),
        principal: Principal::Owner,
    })
}

/// The one place a handshake refusal is written: a `rejected` message names the range this service speaks.
async fn refuse(outbound: &mpsc::Sender<Frame>, error: ErrorBody) {
    let _ = outbound
        .send(Frame::Message(Message::Rejected {
            error,
            supported: SUPPORTED,
        }))
        .await;
}
