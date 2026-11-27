//! The `extensions.points` wire shapes: the method and capability names, and an older or newer peer's reply decoding.

use massorca_protocol::capabilities;
use massorca_protocol::methods::extension_points::{
    self as wire, ExtensionPointOwner, ExtensionPointsList,
};
use serde_json::json;

#[test]
fn the_method_and_capability_names_are_stable() {
    assert_eq!(wire::POINTS, "extensions.points");
    assert_eq!(capabilities::EXTENSION_POINTS, "extension-points/1");
}

#[test]
fn a_reply_round_trips_and_missing_lists_decode_empty() {
    let list = ExtensionPointsList {
        owners: vec![ExtensionPointOwner {
            owner: "ext:acme.all".to_owned(),
            skills: vec!["acme.all/review".to_owned()],
            ..ExtensionPointOwner::default()
        }],
        ..ExtensionPointsList::default()
    };
    let value = serde_json::to_value(&list).expect("encodes");
    assert_eq!(value["owners"][0]["skills"], json!(["acme.all/review"]));
    assert_eq!(
        serde_json::from_value::<ExtensionPointsList>(value).expect("decodes"),
        list
    );

    let sparse: ExtensionPointsList = serde_json::from_value(json!({
        "owners": [{ "owner": "core", "widgets": ["later"] }]
    }))
    .expect("a newer peer's field is ignored");
    assert_eq!(sparse.owners[0].owner, "core");
    assert!(sparse.owners[0].tools.is_empty());
    assert!(sparse.problems.is_empty());
}
