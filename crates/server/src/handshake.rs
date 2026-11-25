//! The server side of the handshake (ADR 0002): `hello` in, `welcome` out. Every refusal goes through `refuse`.

use std::collections::HashSet;
use std::time::Duration;

use massorca_core_store::new_id;
use massorca_protocol::{
    ErrorBody, Frame, MIN_PROTOCOL_VERSION, Message, PROTOCOL_VERSION, ProtocolRange, codes,
    negotiate,
};
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::connection::Origin;
use crate::context::AppState;
use crate::network::{self, LiveGuard};
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
    /// A device connection's place in the live list, registered before `welcome` so a revocation cannot miss it.
    pub(crate) live: Option<LiveGuard>,
}

/// Answers the first frame. `None` closes the connection; a refusal, when one is due, is already queued.
pub(crate) async fn accept(
    inbound: &mut mpsc::Receiver<Frame>,
    outbound: &mpsc::Sender<Frame>,
    state: &AppState,
    max_frame_len: u32,
    origin: &Origin,
) -> Option<Negotiated> {
    let first = first_frame(inbound, outbound).await?;
    let Frame::Message(Message::Hello {
        protocol,
        capabilities: offered,
        auth,
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

    let (principal, live) = match origin {
        Origin::Local => (Principal::Owner, None),
        Origin::Network { client_pin, peer } => {
            match network::authenticate(state, *client_pin, *peer, auth.as_ref()).await {
                Ok(network::Authenticated { principal, live }) => (principal, Some(live)),
                Err(error) => {
                    tracing::info!(%peer, code = %error.code, "a network connection was refused");
                    refuse(outbound, error).await;
                    return None;
                }
            }
        }
    };

    let capabilities = state.registries.capabilities.names();
    let connection_id = new_id();
    let welcome = Message::Welcome {
        protocol: version,
        server: state.server.clone(),
        capabilities: capabilities.clone(),
        connection_id: connection_id.clone(),
        max_frame_len: Some(max_frame_len),
    };
    outbound.send(Frame::Message(welcome)).await.ok()?;
    Some(Negotiated {
        connection_id,
        capabilities: capabilities.into_iter().collect(),
        client_capabilities: offered.into_iter().collect(),
        principal,
        live,
    })
}

/// The one place a handshake refusal is written: a `rejected` message names the range this service speaks.
/// The peer's first frame. A peer silent for `HELLO_TIMEOUT` is refused; `None` closes the connection.
async fn first_frame(
    inbound: &mut mpsc::Receiver<Frame>,
    outbound: &mpsc::Sender<Frame>,
) -> Option<Frame> {
    match timeout(HELLO_TIMEOUT, inbound.recv()).await {
        Ok(first) => first,
        Err(_elapsed) => {
            refuse(
                outbound,
                ErrorBody::new(codes::PROTOCOL_HANDSHAKE, "no hello within 10 seconds"),
            )
            .await;
            None
        }
    }
}

async fn refuse(outbound: &mpsc::Sender<Frame>, error: ErrorBody) {
    let _ = outbound
        .send(Frame::Message(Message::Rejected {
            error,
            supported: SUPPORTED,
        }))
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Instant, sleep};

    fn rejection_code(frame: Option<Frame>) -> Option<String> {
        match frame {
            Some(Frame::Message(Message::Rejected { error, .. })) => Some(error.code),
            _ => None,
        }
    }

    // The clock is paused, so the window is measured exactly however loaded the machine is.
    #[tokio::test(start_paused = true)]
    async fn a_silent_peer_is_refused_when_the_hello_window_ends() {
        let (_peer, mut inbound) = mpsc::channel(1);
        let (outbound, mut sent) = mpsc::channel(1);
        let started = Instant::now();

        assert!(first_frame(&mut inbound, &outbound).await.is_none());
        assert_eq!(started.elapsed(), HELLO_TIMEOUT);
        assert_eq!(
            rejection_code(sent.try_recv().ok()).as_deref(),
            Some(codes::PROTOCOL_HANDSHAKE)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_first_frame_just_inside_the_window_is_read() {
        let (peer, mut inbound) = mpsc::channel(1);
        let (outbound, mut sent) = mpsc::channel(1);
        tokio::spawn(async move {
            sleep(HELLO_TIMEOUT - Duration::from_millis(1)).await;
            let _ = peer
                .send(Frame::Message(Message::Cancel { stream: 1 }))
                .await;
        });

        let first = first_frame(&mut inbound, &outbound).await;
        assert!(
            matches!(first, Some(Frame::Message(Message::Cancel { stream: 1 }))),
            "{first:?}"
        );
        assert!(sent.try_recv().is_err(), "nothing is refused");
    }
}
