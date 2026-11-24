//! The `extensions.update` wire shape (`extensions/1`): optional fields are omitted when unset, and an older peer's
//! request with only `id` still decodes.

use massorca_protocol::methods::extensions::{self, ExtensionUpdateParams};
use serde_json::json;

#[test]
fn update_params_omit_unset_optionals_and_decode_an_id_alone() {
    let consent = ExtensionUpdateParams {
        id: "acme.kit".to_owned(),
        consent: Some("fingerprint".to_owned()),
        sha256: None,
        agent_token: None,
    };
    assert_eq!(
        serde_json::to_value(&consent).expect("serializes"),
        json!({ "id": "acme.kit", "consent": "fingerprint" })
    );

    let bare: ExtensionUpdateParams =
        serde_json::from_value(json!({ "id": "acme.kit" })).expect("deserializes");
    assert_eq!(bare.consent, None);
    assert_eq!(bare.sha256, None);
    assert_eq!(bare.agent_token, None);
}

#[test]
fn update_is_an_extension_method_with_its_wire_name() {
    assert_eq!(extensions::UPDATE, "extensions.update");
}

#[test]
fn a_listing_carries_its_links_and_round_trips() {
    let linked = json!({
        "category": "extensions",
        "website": "https://acme.example",
        "privacy": "https://acme.example/privacy",
        "terms": "https://acme.example/terms"
    });
    let decoded: extensions::Listing = serde_json::from_value(linked).expect("decodes");
    assert_eq!(decoded.website.as_deref(), Some("https://acme.example"));
    assert_eq!(
        decoded.privacy.as_deref(),
        Some("https://acme.example/privacy")
    );
    assert_eq!(decoded.terms.as_deref(), Some("https://acme.example/terms"));
    let value = serde_json::to_value(&decoded).expect("serializes");
    assert_eq!(value["website"], "https://acme.example");
    let again: extensions::Listing = serde_json::from_value(value).expect("decodes its own output");
    assert_eq!(again, decoded);
}

#[test]
fn a_listing_from_an_older_peer_has_no_links_and_sends_none() {
    let decoded: extensions::Listing = serde_json::from_value(json!({ "category": "extensions" }))
        .expect("an older listing decodes");
    assert_eq!(decoded.website, None);
    assert_eq!(decoded.privacy, None);
    assert_eq!(decoded.terms, None);
    let sent = serde_json::to_value(&decoded).expect("serializes");
    assert!(
        sent.get("website").is_none()
            && sent.get("privacy").is_none()
            && sent.get("terms").is_none(),
        "an absent link is not sent"
    );
}
