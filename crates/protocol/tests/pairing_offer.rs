//! The offer string and the `hello.auth` payloads of decision 0041.

use massorca_protocol::methods::devices::Secret;
use massorca_protocol::pairing::{AuthRequest, Offer, OfferError};

fn offer() -> Offer {
    Offer {
        server: "vps".to_owned(),
        addresses: vec!["100.64.0.1:7347".to_owned()],
        pin: "sha256:AAAA".to_owned(),
        secret: Secret::new("throwaway-test-secret"),
        expires_at_ms: 1_000,
        scopes: vec!["read".to_owned()],
    }
}

#[test]
fn an_offer_round_trips_and_never_shows_its_secret() {
    let encoded = offer().encode();
    assert!(encoded.expose().starts_with("mop1."));
    assert_eq!(Offer::decode(encoded.expose()).expect("decodes"), offer());
    assert!(!format!("{:?}", offer()).contains("throwaway"));
    assert!(!format!("{encoded:?}").contains("mop1"));
}

#[test]
fn a_damaged_or_foreign_offer_is_refused() {
    assert_eq!(Offer::decode("hello"), Err(OfferError::NotAnOffer));
    assert_eq!(Offer::decode("mop1.!!!"), Err(OfferError::Damaged));
    let long = format!("mop1.{}", "A".repeat(5000));
    assert_eq!(Offer::decode(&long), Err(OfferError::Damaged));
    let mut empty = offer();
    empty.addresses.clear();
    assert_eq!(
        Offer::decode(empty.encode().expose()),
        Err(OfferError::Damaged)
    );
}

#[test]
fn auth_requests_travel_in_hello() {
    let pair = AuthRequest::Pair {
        secret: Secret::new("s"),
        name: "laptop".to_owned(),
    };
    let auth = pair.to_auth();
    assert_eq!(auth.as_value()["method"], "pair");
    assert_eq!(AuthRequest::from_auth(Some(&auth)), Some(pair));
    let device = AuthRequest::Device {}.to_auth();
    assert_eq!(device.as_value()["method"], "device");
    assert_eq!(
        AuthRequest::from_auth(Some(&device)),
        Some(AuthRequest::Device {})
    );
    let future = massorca_protocol::Auth::from(serde_json::json!({"method": "passkey"}));
    assert_eq!(
        AuthRequest::from_auth(Some(&future)),
        Some(AuthRequest::Unknown)
    );
    assert_eq!(AuthRequest::from_auth(None), None);
}
