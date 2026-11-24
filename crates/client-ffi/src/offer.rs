//! Reading a pairing offer the app scanned or pasted, before any network use (M-PA-02).

use std::time::{SystemTime, UNIX_EPOCH};

use massorca_protocol::pairing::Offer;

use crate::error::FfiError;

/// What an offer says, without its secret: enough for the confirm step.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct OfferSummary {
    /// The server's own name.
    pub server: String,
    /// Addresses the phone will try, `host:port`.
    pub addresses: Vec<String>,
    pub expires_at_ms: i64,
    /// What the new device will be allowed to do.
    pub scopes: Vec<String>,
}

/// Checks a scanned or pasted pairing code. Refuses text that is not an offer, a damaged one and an expired one.
#[uniffi::export]
pub fn parse_offer(text: String) -> Result<OfferSummary, FfiError> {
    let offer = decode(&text)?;
    Ok(OfferSummary {
        server: offer.server,
        addresses: offer.addresses,
        expires_at_ms: offer.expires_at_ms,
        scopes: offer.scopes,
    })
}

/// The offer in `text`, refused once expired.
pub(crate) fn decode(text: &str) -> Result<Offer, FfiError> {
    let offer = Offer::decode(text).map_err(|_| FfiError::InvalidOffer {
        message: "Not a valid pairing code.".to_owned(),
    })?;
    if offer.expires_at_ms <= now_ms() {
        return Err(FfiError::InvalidOffer {
            message: "This pairing code has expired. Make a new one on the host.".to_owned(),
        });
    }
    Ok(offer)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::devices::Secret;
    use massorca_protocol::pairing::Offer;

    use super::{now_ms, parse_offer};

    fn offer(expires_at_ms: i64) -> String {
        Offer {
            server: "vps".to_owned(),
            addresses: vec!["100.64.0.10:7347".to_owned()],
            pin: "sha256:placeholder".to_owned(),
            secret: Secret::new("placeholder-secret"),
            expires_at_ms,
            scopes: vec!["read".to_owned()],
        }
        .encode()
        .expose()
        .to_owned()
    }

    #[test]
    fn a_valid_offer_is_summarised_without_its_secret() {
        let expires = now_ms() + 60_000;
        let summary = parse_offer(format!("  {}\n", offer(expires))).expect("valid");
        assert_eq!(summary.server, "vps");
        assert_eq!(summary.addresses, ["100.64.0.10:7347"]);
        assert_eq!(summary.expires_at_ms, expires);
        assert_eq!(summary.scopes, ["read"]);
    }

    #[test]
    fn other_text_a_damaged_offer_and_an_expired_one_are_refused() {
        let good = offer(now_ms() + 60_000);
        for text in [
            "hello".to_owned(),
            format!("{}!!", &good[..good.len() / 2]),
            offer(now_ms() - 1),
        ] {
            let refused = parse_offer(text).expect_err("refused");
            assert_eq!(refused.code(), "invalid_offer", "{refused:?}");
        }
        let expired = parse_offer(offer(now_ms() - 1)).expect_err("refused");
        assert!(expired.to_string().contains("expired"), "{expired}");
    }
}
