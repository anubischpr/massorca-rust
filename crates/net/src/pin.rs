use std::fmt;
use std::str::FromStr;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::NetError;

const PREFIX: &str = "sha256:";

/// SHA-256 of a certificate's SubjectPublicKeyInfo DER, shown as `sha256:<base64url, no padding>`. A pin is public.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pin([u8; 32]);

impl Pin {
    /// The pin of a DER certificate's public key.
    pub fn of_certificate(cert_der: &[u8]) -> Result<Pin, NetError> {
        let (_, cert) = x509_parser::parse_x509_certificate(cert_der)
            .map_err(|error| NetError::Certificate(error.to_string()))?;
        Ok(Self::of_spki(cert.tbs_certificate.subject_pki.raw))
    }

    pub(crate) fn of_spki(spki_der: &[u8]) -> Pin {
        let digest = ring::digest::digest(&ring::digest::SHA256, spki_der);
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(digest.as_ref());
        Pin(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for Pin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{PREFIX}{}", URL_SAFE_NO_PAD.encode(self.0))
    }
}

impl fmt::Debug for Pin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Pin({self})")
    }
}

impl FromStr for Pin {
    type Err = NetError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let encoded = text
            .strip_prefix(PREFIX)
            .ok_or_else(|| NetError::InvalidPin(format!("expected the `{PREFIX}` prefix")))?;
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|error| NetError::InvalidPin(error.to_string()))?;
        let bytes: [u8; 32] = bytes.try_into().map_err(|bytes: Vec<u8>| {
            NetError::InvalidPin(format!("expected 32 bytes, got {}", bytes.len()))
        })?;
        Ok(Pin(bytes))
    }
}
