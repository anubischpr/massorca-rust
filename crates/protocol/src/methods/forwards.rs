//! Port forwards (capability `port-forward/1`, decision 0131): a client tunnels TCP connections to a loopback port on
//! the host over its paired connection.
//!
//! `host.forward` opens a forward and keeps it while its event stream is open. Each local connection then opens one
//! `host.forward_connect` stream, whose stream-bytes frames go both ways and carry [`Chunk`]s: data, end of data, and
//! acknowledgements that keep each direction within [`WINDOW_CHUNKS`] unacknowledged chunks.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

pub const FORWARD: &str = "host.forward";
pub const FORWARD_CONNECT: &str = "host.forward_connect";
pub const FORWARDS: &str = "host.forwards";
pub const FORWARD_STOP: &str = "host.forward_stop";

/// Largest data chunk either side sends.
pub const MAX_CHUNK: usize = 32 * 1024;
/// Data chunks one side may send before the other acknowledges them.
pub const WINDOW_CHUNKS: u32 = 16;
/// Forwards one connection may hold open.
pub const MAX_FORWARDS_PER_CONNECTION: usize = 16;
/// Connections one forward may carry at a time.
pub const MAX_CONNECTIONS_PER_FORWARD: usize = 64;
/// A tunnel that carries nothing either way for this long is closed.
pub const IDLE_TIMEOUT_SECS: u64 = 30 * 60;

/// Params of `host.forward`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardParams {
    /// The port on the host.
    pub port: u16,
    /// A loopback address on the host. Without it, `127.0.0.1` is tried, then `::1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<IpAddr>,
    /// The client's local port, shown in listings only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_port: Option<u16>,
}

/// One open forward, as the host sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardInfo {
    pub id: String,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<IpAddr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_port: Option<u16>,
    /// The device that opened it; none for the host's owner on its local socket.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    pub opened_at_ms: i64,
    /// Connections open now.
    #[serde(default)]
    pub connections: u32,
    /// Connections carried since the forward opened.
    #[serde(default)]
    pub total_connections: u64,
    /// Bytes written to the host's port, counted when each connection ends.
    #[serde(default)]
    pub bytes_to_host: u64,
    /// Bytes read from the host's port, counted when each connection ends.
    #[serde(default)]
    pub bytes_from_host: u64,
}

/// Events of the `host.forward` stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ForwardEvent {
    /// First event: the forward is open.
    Opened { forward: ForwardInfo },
    /// Last event before the stream ends when `host.forward_stop` ended it.
    Stopped { forward: ForwardInfo },
}

/// Params of `host.forward_connect`: the forward, opened on the same connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardConnectParams {
    pub forward: String,
}

/// Result of `host.forwards`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardList {
    pub forwards: Vec<ForwardInfo>,
}

/// Params of `host.forward_stop`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardStopParams {
    pub forward: String,
}

/// Result of `host.forward_stop`: the forward as it was when stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardStopped {
    pub forward: ForwardInfo,
}

const TAG_DATA: u8 = 0;
const TAG_EOF: u8 = 1;
const TAG_ACK: u8 = 2;

/// One stream-bytes payload of a `host.forward_connect` stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chunk<'a> {
    /// Bytes of the connection, at most [`MAX_CHUNK`].
    Data(&'a [u8]),
    /// The sender reads no more from its socket: the receiver shuts its socket's write side.
    Eof,
    /// This many data chunks were written to the receiver's socket.
    Ack(u32),
}

/// Why a payload is not a [`Chunk`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChunkError {
    #[error("an empty forward payload")]
    Empty,
    #[error("unknown forward payload tag {0}")]
    UnknownTag(u8),
    #[error("a forward data chunk of {0} bytes is over the limit")]
    TooLarge(usize),
    #[error("a malformed forward payload")]
    Malformed,
}

impl Chunk<'_> {
    pub fn encode(self) -> Vec<u8> {
        match self {
            Self::Data(bytes) => {
                let mut payload = Vec::with_capacity(bytes.len() + 1);
                payload.push(TAG_DATA);
                payload.extend_from_slice(bytes);
                payload
            }
            Self::Eof => vec![TAG_EOF],
            Self::Ack(count) => {
                let mut payload = vec![TAG_ACK];
                payload.extend_from_slice(&count.to_be_bytes());
                payload
            }
        }
    }
}

impl<'a> Chunk<'a> {
    pub fn decode(payload: &'a [u8]) -> Result<Self, ChunkError> {
        let (&tag, rest) = payload.split_first().ok_or(ChunkError::Empty)?;
        match tag {
            TAG_DATA if rest.is_empty() => Err(ChunkError::Malformed),
            TAG_DATA if rest.len() > MAX_CHUNK => Err(ChunkError::TooLarge(rest.len())),
            TAG_DATA => Ok(Self::Data(rest)),
            TAG_EOF if rest.is_empty() => Ok(Self::Eof),
            TAG_ACK => {
                let count: [u8; 4] = rest.try_into().map_err(|_| ChunkError::Malformed)?;
                Ok(Self::Ack(u32::from_be_bytes(count)))
            }
            TAG_EOF => Err(ChunkError::Malformed),
            other => Err(ChunkError::UnknownTag(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Chunk, ChunkError, ForwardEvent, ForwardInfo, ForwardParams, MAX_CHUNK};

    #[test]
    fn chunks_round_trip_and_bad_payloads_are_refused() {
        for chunk in [Chunk::Data(b"hello"), Chunk::Eof, Chunk::Ack(3)] {
            assert_eq!(Chunk::decode(&chunk.encode()), Ok(chunk));
        }
        assert_eq!(Chunk::decode(&[]), Err(ChunkError::Empty));
        assert_eq!(Chunk::decode(&[0]), Err(ChunkError::Malformed));
        assert_eq!(Chunk::decode(&[1, 0]), Err(ChunkError::Malformed));
        assert_eq!(Chunk::decode(&[2, 0, 1]), Err(ChunkError::Malformed));
        assert_eq!(Chunk::decode(&[9]), Err(ChunkError::UnknownTag(9)));
        let big = vec![0; MAX_CHUNK + 2];
        assert_eq!(
            Chunk::decode(&big),
            Err(ChunkError::TooLarge(MAX_CHUNK + 1))
        );
    }

    #[test]
    fn params_and_events_keep_their_wire_shape() {
        let params: ForwardParams = serde_json::from_value(json!({ "port": 3000 })).unwrap();
        assert_eq!(params.address, None);
        let info = ForwardInfo {
            id: "f-1".to_owned(),
            port: 3000,
            address: None,
            local_port: Some(8080),
            device_id: None,
            opened_at_ms: 5,
            connections: 0,
            total_connections: 0,
            bytes_to_host: 0,
            bytes_from_host: 0,
        };
        let event = serde_json::to_value(ForwardEvent::Opened {
            forward: info.clone(),
        })
        .unwrap();
        assert_eq!(event["type"], "opened");
        assert_eq!(event["forward"]["id"], "f-1");
        assert!(event["forward"].get("address").is_none(), "{event}");
        assert_eq!(
            serde_json::from_value::<ForwardEvent>(event).unwrap(),
            ForwardEvent::Opened { forward: info }
        );
    }
}
