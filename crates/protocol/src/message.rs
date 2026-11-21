//! Wire messages: one JSON object per frame, tagged by `type`.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Name and version of a peer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerInfo {
    pub name: String,
    pub version: String,
}

/// Inclusive range of protocol versions a peer speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolRange {
    pub min: u32,
    pub max: u32,
}

/// Failure detail. `code` is a stable dotted string; `message` is for humans only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl ErrorBody {
    /// An error without `data`.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            data: None,
        }
    }
}

/// One protocol message. Unknown `type`s decode to [`Message::Unknown`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Hello {
        protocol: ProtocolRange,
        client: PeerInfo,
        #[serde(default)]
        capabilities: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        auth: Option<Value>,
    },
    Welcome {
        protocol: u32,
        server: PeerInfo,
        #[serde(default)]
        capabilities: Vec<String>,
        connection_id: String,
    },
    /// Sent instead of `welcome` when the handshake fails; the server then closes.
    Rejected {
        error: ErrorBody,
        /// The protocol range the server speaks.
        supported: ProtocolRange,
    },
    Request {
        id: u64,
        method: String,
        #[serde(default)]
        params: Value,
    },
    Response {
        id: u64,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "present_result"
        )]
        result: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<ErrorBody>,
    },
    Event {
        stream: u64,
        data: Value,
    },
    End {
        stream: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<ErrorBody>,
    },
    Cancel {
        stream: u64,
    },
    /// A `type` this build does not know; receivers ignore it.
    #[serde(other)]
    Unknown,
}

impl Message {
    /// A successful response. The result stays set even when it is `null`.
    pub fn response_ok(id: u64, result: Value) -> Self {
        Self::Response {
            id,
            result: Some(result),
            error: None,
        }
    }

    /// A failed response.
    pub fn response_err(id: u64, error: ErrorBody) -> Self {
        Self::Response {
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// Keeps an explicit `null` result as `Some(Value::Null)`; a plain `Option` would read it as absent.
fn present_result<'de, D>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: Deserializer<'de>,
{
    Value::deserialize(deserializer).map(Some)
}
