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

#[test]
fn rollback_params_omit_unset_optionals_and_decode_an_id_alone() {
    let consented = extensions::ExtensionRollbackParams {
        id: "acme.kit".to_owned(),
        consent: Some("fingerprint".to_owned()),
        agent_token: None,
    };
    assert_eq!(
        serde_json::to_value(&consented).expect("serializes"),
        json!({ "id": "acme.kit", "consent": "fingerprint" })
    );

    let bare: extensions::ExtensionRollbackParams =
        serde_json::from_value(json!({ "id": "acme.kit" })).expect("deserializes");
    assert_eq!(bare.consent, None);
    assert_eq!(bare.agent_token, None);
}

#[test]
fn rollback_is_an_extension_method_with_its_wire_name() {
    assert_eq!(extensions::ROLLBACK, "extensions.rollback");
}

#[test]
fn a_run_log_decodes_newer_values_as_unknown_and_omits_unset_output() {
    let log: extensions::ExtensionLog = serde_json::from_value(json!({
        "id": "acme.kit",
        "runs": [
            { "at_ms": 5, "action": "rollback", "from_version": "1.3.0", "to_version": "1.2.0", "outcome": "ok" },
            { "at_ms": 6, "action": "prune", "outcome": "partly", "output": ["line"] }
        ]
    }))
    .expect("decodes");

    assert_eq!(log.runs[0].action, extensions::ExtensionRunAction::Rollback);
    assert_eq!(log.runs[0].outcome, extensions::ExtensionRunOutcome::Ok);
    assert!(log.runs[0].output.is_empty());
    assert_eq!(log.runs[1].action, extensions::ExtensionRunAction::Unknown);
    assert_eq!(
        log.runs[1].outcome,
        extensions::ExtensionRunOutcome::Unknown
    );
    assert_eq!(log.runs[1].output, ["line"]);
    let value = serde_json::to_value(&log.runs[0]).expect("serializes");
    assert!(value.get("output").is_none());
    assert!(value.get("to_version").is_some());
}

#[test]
fn the_run_log_is_an_extension_method_with_its_wire_name() {
    assert_eq!(extensions::LOG, "extensions.log");
}
