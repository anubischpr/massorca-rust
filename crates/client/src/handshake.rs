//! The client side of the handshake (ADR 0002): `hello` out, `welcome` in. A refusal from the service becomes `ClientError::Handshake`.

use std::time::Duration;

use massorca_protocol::{
    Auth, ErrorBody, Frame, MIN_PROTOCOL_VERSION, Message, PROTOCOL_VERSION, PeerInfo,
    ProtocolRange, capabilities, codes,
};
use tokio::time::timeout;

use crate::error::ClientError;
use crate::transport::FrameIo;

/// How long the service may take to answer `hello`.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// The capabilities the typed helpers use. The service records them and offers its own list in `welcome` either way.
const OFFERED: [&str; 5] = [
    capabilities::SERVICE,
    capabilities::EVENTS,
    capabilities::CONFIG,
    capabilities::BINARY_FRAMES,
    capabilities::COMMANDS,
];

/// What the service answered in `welcome`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Welcome {
    pub protocol: u32,
    pub server: PeerInfo,
    pub capabilities: Vec<String>,
    pub connection_id: String,
    /// The largest frame body the service accepts on this connection.
    pub max_frame_len: u32,
}

/// Sends `hello` through the transport and reads the answer. The transport is closed by the caller on failure.
pub(crate) async fn exchange(
    io: &mut FrameIo,
    client: &PeerInfo,
    auth: Option<Auth>,
) -> Result<Welcome, ClientError> {
    let hello = Message::Hello {
        protocol: ProtocolRange {
            min: MIN_PROTOCOL_VERSION,
            max: PROTOCOL_VERSION,
        },
        client: client.clone(),
        capabilities: OFFERED.iter().map(|name| (*name).to_owned()).collect(),
        auth,
    };
    io.outbound
        .send(hello)
        .await
        .map_err(|_| ClientError::Disconnected)?;

    let answer = match timeout(HANDSHAKE_TIMEOUT, io.inbound.recv()).await {
        Ok(answer) => answer,
        Err(_elapsed) => {
            return Err(ClientError::Handshake {
                error: Box::new(ErrorBody::new(
                    codes::PROTOCOL_HANDSHAKE,
                    "the service did not answer hello in time",
                )),
                supported: None,
            });
        }
    };
    match answer {
        Some(Frame::Message(Message::Welcome {
            protocol,
            server,
            capabilities,
            connection_id,
            max_frame_len,
        })) => {
            if !(MIN_PROTOCOL_VERSION..=PROTOCOL_VERSION).contains(&protocol) {
                return Err(ClientError::Protocol(format!(
                    "the service chose protocol {protocol}, which this client does not speak"
                )));
            }
            Ok(Welcome {
                protocol,
                server,
                capabilities,
                connection_id,
                // Never above what this transport carries, whatever the service announced.
                max_frame_len: max_frame_len.map_or(io.max_frame_len, |announced| {
                    announced.min(io.max_frame_len)
                }),
            })
        }
        Some(Frame::Message(Message::Rejected { error, supported })) => {
            Err(ClientError::Handshake {
                error: Box::new(error),
                supported: Some(supported),
            })
        }
        Some(_) => Err(ClientError::Protocol(
            "the service answered hello with something other than welcome".to_owned(),
        )),
        None => Err(ClientError::Disconnected),
    }
}
