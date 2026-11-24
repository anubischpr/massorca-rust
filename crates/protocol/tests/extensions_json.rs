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
