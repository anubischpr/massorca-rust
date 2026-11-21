//! Wire messages: one JSON object per frame, tagged by `type`.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use serde_json::Value;
use std::fmt;

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

/// Credentials a client sends in `hello`. `Debug` never prints them.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Auth(Value);

impl Auth {
    pub fn as_value(&self) -> &Value {
        &self.0
    }

    pub fn into_value(self) -> Value {
        self.0
    }
}

impl From<Value> for Auth {
    fn from(value: Value) -> Self {
        Self(value)
    }
}

impl fmt::Debug for Auth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// One protocol message. Unknown `type`s decode to [`Message::Unknown`].
// `remote = "Self"` turns the derives into inherent fns, so the trait impls below can validate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "Self", tag = "type", rename_all = "snake_case")]
pub enum Message {
    Hello {
        protocol: ProtocolRange,
        client: PeerInfo,
        #[serde(default)]
        capabilities: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        auth: Option<Auth>,
    },
    Welcome {
        protocol: u32,
        server: PeerInfo,
        #[serde(default)]
        capabilities: Vec<String>,
        connection_id: String,
        /// Largest frame body the server accepts; absent means the 0002 default.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_frame_len: Option<u32>,
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

impl Serialize for Message {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for Message {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let message = Self::deserialize(deserializer)?;
        if let Self::Response { result, error, .. } = &message
            && result.is_some() == error.is_some()
        {
            return Err(D::Error::custom(
                "a response carries exactly one of `result` or `error`",
            ));
        }
        Ok(message)
    }
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
