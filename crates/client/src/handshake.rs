//! The client side of the handshake (ADR 0002): `hello` out, `welcome` in. A refusal from the service becomes `ClientError::Handshake`.

use std::time::Duration;

use interprocess::local_socket::tokio::Stream;
use massorca_protocol::{
    ErrorBody, FrameError, MAX_FRAME_LEN_LOCAL, MIN_PROTOCOL_VERSION, Message, PROTOCOL_VERSION,
    PeerInfo, ProtocolRange, capabilities, codes, read_message, write_message,
};
use tokio::time::timeout;

use crate::error::ClientError;

/// How long the service may take to answer `hello`.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// The capabilities the typed helpers use. The service records them and offers its own list in `welcome` either way.
const OFFERED: [&str; 3] = [
    capabilities::SERVICE,
    capabilities::EVENTS,
    capabilities::CONFIG,
];

/// What the service answered in `welcome`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Welcome {
    pub protocol: u32,
    pub server: PeerInfo,
    pub capabilities: Vec<String>,
    pub connection_id: String,
}

/// Sends `hello` and reads the answer.
pub(crate) async fn exchange(
    stream: &mut Stream,
    client: &PeerInfo,
) -> Result<Welcome, ClientError> {
    let hello = Message::Hello {
        protocol: ProtocolRange {
            min: MIN_PROTOCOL_VERSION,
            max: PROTOCOL_VERSION,
        },
        client: client.clone(),
        capabilities: OFFERED.iter().map(|name| (*name).to_owned()).collect(),
        auth: None,
    };
    write_message(stream, &hello).await.map_err(frame_error)?;

    let answer = match timeout(HANDSHAKE_TIMEOUT, read_message(stream, MAX_FRAME_LEN_LOCAL)).await {
        Ok(read) => read.map_err(frame_error)?,
        // The connection is dropped with the error, so a late answer cannot reach anyone.
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
        Some(Message::Welcome {
            protocol,
            server,
            capabilities,
            connection_id,
            max_frame_len: _,
        }) => {
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
            })
        }
        Some(Message::Rejected { error, supported }) => Err(ClientError::Handshake {
            error: Box::new(error),
            supported: Some(supported),
        }),
        Some(_) => Err(ClientError::Protocol(
            "the service answered hello with something other than welcome".to_owned(),
        )),
        None => Err(ClientError::Disconnected),
    }
}

fn frame_error(error: FrameError) -> ClientError {
    match error {
        FrameError::Io(_) => ClientError::Disconnected,
        other => ClientError::Protocol(other.to_string()),
    }
}
