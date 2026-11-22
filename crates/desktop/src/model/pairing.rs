//! The Pairing page (D-PR-01, D-PR-02): an offer this computer makes for another device, and a pairing
//! this computer accepts from an offer another host made. The offer is a secret: the page shows it and
//! never prints it. Plain data and rules, free of GPUI.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use massorca_protocol::methods::devices::PairingCreated;

/// The line under a new offer, as the CLI prints it (decision 0041).
pub const OFFER_WARNING: &str = "It works once. Treat it like a password.";

/// Where the offer this computer makes stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OfferState {
    #[default]
    Idle,
    /// The service is asked for an offer.
    Making,
    Made(OfferView),
    /// The service refused, with its sentence.
    Refused(String),
}

/// An offer as the page shows it.
#[derive(Clone, PartialEq, Eq)]
pub struct OfferView {
    pub offer: String,
    pub expires_at_ms: i64,
    pub scopes: String,
    pub addresses: String,
}

impl fmt::Debug for OfferView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OfferView")
            .field("offer", &"<redacted>")
            .field("expires_at_ms", &self.expires_at_ms)
            .field("scopes", &self.scopes)
            .field("addresses", &self.addresses)
            .finish()
    }
}

impl OfferView {
    pub fn from_created(created: &PairingCreated) -> Self {
        Self {
            offer: created.offer.expose().to_owned(),
            expires_at_ms: created.expires_at_ms,
            scopes: created.scopes.join(", "),
            addresses: created.addresses.join(", "),
        }
    }
}

/// Where an accepted offer stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum AcceptState {
    #[default]
    Idle,
    /// The pairing is under way.
    Pairing,
    /// The host was paired and is listed under this name.
    Paired(String),
    /// The service or the host refused, with its sentence.
    Refused(String),
}

/// What the Pairing page holds between frames.
#[derive(Clone, Debug, Default)]
pub struct PairingState {
    pub offer: OfferState,
    pub accept: AcceptState,
    /// The offer field is cleared on the next frame, which has the window to clear it with.
    pub clear_offer_field: bool,
}

/// "Make offer" can be pressed unless an offer is being made already.
pub fn can_make_offer(offer: &OfferState) -> bool {
    *offer != OfferState::Making
}

/// "Pair" can be pressed when an offer is typed and no pairing is under way.
pub fn can_pair(typed_offer: &str, accept: &AcceptState) -> bool {
    !typed_offer.trim().is_empty() && *accept != AcceptState::Pairing
}

/// The name sent with an accepted offer: the typed name, or none so the server's own name is used.
pub fn name_or_none(typed: &str) -> Option<String> {
    let name = typed.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// "Expires in 10 min.", the time left in whole minutes rounded up, or "Expired" once it has passed.
pub fn expiry_text(expires_at_ms: i64, now_ms: i64) -> String {
    let left = expires_at_ms.saturating_sub(now_ms);
    if left <= 0 {
        return "Expired. Make a new offer.".to_owned();
    }
    let minutes = left.saturating_add(59_999) / 60_000;
    format!("Expires in {minutes} min.")
}

/// The current Unix time in milliseconds, for the expiry line.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_expiry_counts_whole_minutes_rounded_up() {
        assert_eq!(expiry_text(600_000, 0), "Expires in 10 min.");
        assert_eq!(expiry_text(600_001, 0), "Expires in 11 min.");
        assert_eq!(expiry_text(1, 0), "Expires in 1 min.");
    }

    #[test]
    fn an_offer_past_its_time_says_expired_and_offers_a_new_one() {
        assert_eq!(expiry_text(5_000, 5_000), "Expired. Make a new offer.");
        assert_eq!(expiry_text(4_000, 5_000), "Expired. Make a new offer.");
    }

    #[test]
    fn an_offer_cannot_be_made_while_one_is_being_made() {
        assert!(can_make_offer(&OfferState::Idle));
        assert!(can_make_offer(&OfferState::Refused("no".into())));
        assert!(!can_make_offer(&OfferState::Making));
    }

    #[test]
    fn pairing_needs_a_typed_offer_and_no_pairing_under_way() {
        assert!(!can_pair("  ", &AcceptState::Idle));
        assert!(can_pair("offer-text", &AcceptState::Idle));
        assert!(!can_pair("offer-text", &AcceptState::Pairing));
    }

    #[test]
    fn a_blank_name_means_the_servers_own_name() {
        assert_eq!(name_or_none("  "), None);
        assert_eq!(name_or_none(" vps "), Some("vps".to_owned()));
    }

    #[test]
    fn an_offer_never_shows_its_secret_in_debug_output() {
        let view = OfferView {
            offer: "secret-offer".into(),
            expires_at_ms: 1,
            scopes: "read".into(),
            addresses: "a".into(),
        };
        assert!(!format!("{view:?}").contains("secret-offer"));
    }
}
