//! The pairing offer string and the `hello.auth` payloads of network connections (decision 0041).

use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};

use crate::message::Auth;
use crate::methods::devices::Secret;

/// The prefix of every offer string; the digit is the offer format version.
pub const OFFER_PREFIX: &str = "mop1.";

/// Offers longer than this are refused before decoding.
const MAX_OFFER_LEN: usize = 4096;

/// What a pairing offer carries. `Debug` redacts the secret.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    /// The server's name, a default for the host name on the client.
    pub server: String,
    /// Addresses to try in order, `host:port`.
    pub addresses: Vec<String>,
    /// `sha256:<base64url>` of the server's public key.
    pub pin: String,
    pub secret: Secret,
    pub expires_at_ms: i64,
    pub scopes: Vec<String>,
}

impl fmt::Debug for Offer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Offer")
            .field("server", &self.server)
            .field("addresses", &self.addresses)
            .field("pin", &self.pin)
            .field("secret", &self.secret)
            .field("expires_at_ms", &self.expires_at_ms)
            .field("scopes", &self.scopes)
            .finish()
    }
}

/// Why an offer string could not be read. Never echoes the string.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OfferError {
    #[error("this is not a MassOrca pairing offer")]
    NotAnOffer,
    #[error("the pairing offer is damaged")]
    Damaged,
}

impl Offer {
    /// The string handed to the new device: the prefix, then base64url JSON.
    pub fn encode(&self) -> Secret {
        let json = serde_json::to_vec(self).expect("offers always serialize");
        Secret::new(format!("{OFFER_PREFIX}{}", URL_SAFE_NO_PAD.encode(json)))
    }

    pub fn decode(text: &str) -> Result<Self, OfferError> {
        let text = text.trim();
        let body = text
            .strip_prefix(OFFER_PREFIX)
            .ok_or(OfferError::NotAnOffer)?;
        if text.len() > MAX_OFFER_LEN {
            return Err(OfferError::Damaged);
        }
        let json = URL_SAFE_NO_PAD
            .decode(body)
            .map_err(|_| OfferError::Damaged)?;
        let offer: Self = serde_json::from_slice(&json).map_err(|_| OfferError::Damaged)?;
        if offer.addresses.is_empty() || offer.secret.expose().is_empty() {
            return Err(OfferError::Damaged);
        }
        Ok(offer)
    }
}

/// What a network client sends in `hello.auth`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum AuthRequest {
    /// A paired device; its identity is the TLS client certificate.
    Device {},
    /// Pairing with the secret of an offer; `name` is how the server lists this device.
    Pair { secret: Secret, name: String },
    /// A method this build does not know.
    #[serde(other)]
    Unknown,
}

impl AuthRequest {
    pub fn to_auth(&self) -> Auth {
        Auth::from(serde_json::to_value(self).expect("auth requests always serialize"))
    }

    /// Reads `hello.auth`; `None` when it is absent or not an object this build understands.
    pub fn from_auth(auth: Option<&Auth>) -> Option<Self> {
        auth.and_then(|auth| serde_json::from_value(auth.as_value().clone()).ok())
    }
}
