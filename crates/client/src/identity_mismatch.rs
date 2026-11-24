//! A host that presents another server key than the one this client trusts (REM-04): which keys, and how to accept the
//! new one deliberately.

use serde_json::{Value, json};

/// The code the CLI and the desktop report it under; stable, scripts may match it.
pub const IDENTITY_MISMATCH_CODE: &str = "host.identity_mismatch";

/// Where the trusted key came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustedFrom {
    /// The key pinned when the host was paired.
    Pairing,
    /// The key the pairing offer names, before the host is stored.
    Offer,
}

/// The keys of a refused connection. Never retried: only a deliberate new pairing trusts another key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityMismatch {
    /// The host's name on this client.
    pub host: String,
    pub trusted_from: TrustedFrom,
    /// `sha256:<base64url>` of the trusted key.
    pub expected: String,
    /// The key the server presented, when its certificate could be read.
    pub actual: Option<String>,
}

impl IdentityMismatch {
    /// What changed, and what to do about it.
    pub fn message(&self) -> String {
        let host = &self.host;
        let actual = self
            .actual
            .as_deref()
            .unwrap_or("a certificate that could not be read");
        match self.trusted_from {
            TrustedFrom::Pairing => format!(
                "{host} presented a different server key than the one paired with (paired {}, now {actual}); refusing \
                 to connect. If that server was reinstalled or its key replaced, check the new key on the server \
                 (`massorca network status`), then pair again: `massorca host remove {host} --keep-device`, and accept \
                 a new offer from it with `massorca pair accept <offer> --name {host}` or on the desktop's Pairing \
                 page. Otherwise do not connect: something may be between you and the server",
                self.expected
            ),
            TrustedFrom::Offer => format!(
                "the server at the offer's address presented a different key than the offer names (offer {}, server \
                 {actual}); nothing was paired. Make a new offer on that server and accept it; if the offer is new, \
                 something may be between you and the server",
                self.expected
            ),
        }
    }

    /// The error data: the host and both keys, so a script or the app can show them without parsing the message.
    pub fn data(&self) -> Value {
        json!({
            "host": self.host,
            "trusted_from": match self.trusted_from {
                TrustedFrom::Pairing => "pairing",
                TrustedFrom::Offer => "offer",
            },
            "expected": self.expected,
            "actual": self.actual,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{IdentityMismatch, TrustedFrom};

    fn changed(trusted_from: TrustedFrom) -> IdentityMismatch {
        IdentityMismatch {
            host: "vps".to_owned(),
            trusted_from,
            expected: "sha256:old".to_owned(),
            actual: Some("sha256:new".to_owned()),
        }
    }

    #[test]
    fn a_changed_paired_key_names_both_keys_and_the_way_to_pair_again() {
        let message = changed(TrustedFrom::Pairing).message();
        for part in [
            "sha256:old",
            "sha256:new",
            "massorca network status",
            "massorca host remove vps --keep-device",
            "massorca pair accept <offer> --name vps",
            "Pairing page",
        ] {
            assert!(message.contains(part), "{part:?} missing from {message:?}");
        }
        assert_eq!(
            changed(TrustedFrom::Pairing).data(),
            serde_json::json!({
                "host": "vps",
                "trusted_from": "pairing",
                "expected": "sha256:old",
                "actual": "sha256:new",
            })
        );
    }

    #[test]
    fn an_offer_whose_key_the_server_does_not_hold_asks_for_a_new_offer() {
        let message = changed(TrustedFrom::Offer).message();
        assert!(message.contains("Make a new offer"), "{message}");
        assert!(!message.contains("host remove"), "{message}");
        let unreadable = IdentityMismatch {
            actual: None,
            ..changed(TrustedFrom::Pairing)
        };
        assert!(unreadable.message().contains("could not be read"));
        assert_eq!(unreadable.data()["actual"], serde_json::Value::Null);
    }
}
