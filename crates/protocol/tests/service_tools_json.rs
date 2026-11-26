//! The wire shape of `service.tools` and the fallbacks for kinds and states a client does not know.

use massorca_protocol::methods::service::{ToolInfo, ToolKind, ToolList, ToolState, ToolsParams};
use serde_json::json;

#[test]
fn the_tool_list_has_the_documented_wire_shape() {
    let list = ToolList {
        tools: vec![ToolInfo {
            id: "gh".to_owned(),
            kind: ToolKind::Forge,
            state: ToolState::Present,
            version: Some("2.40.1".to_owned()),
            path: None,
        }],
    };

    let wire = serde_json::to_value(&list).expect("serialize the tool list");

    assert_eq!(
        wire,
        json!({"tools": [{"id": "gh", "kind": "forge", "state": "present", "version": "2.40.1", "path": null}]})
    );
    let back: ToolList = serde_json::from_value(wire).expect("decode the tool list");
    assert_eq!(back, list);
}

#[test]
fn a_kind_or_state_this_client_does_not_know_decodes_as_other() {
    let list: ToolList = serde_json::from_value(json!({
        "tools": [{"id": "cargo", "kind": "toolchain", "state": "stale", "version": null, "path": null}]
    }))
    .expect("decode a newer service's list");

    assert_eq!(list.tools[0].id, "cargo");
    assert_eq!(list.tools[0].kind, ToolKind::Other);
    assert_eq!(list.tools[0].state, ToolState::Other);
}

#[test]
fn refresh_defaults_to_false_when_absent() {
    let absent: ToolsParams = serde_json::from_value(json!({})).expect("empty params");
    assert!(!absent.refresh);
    let given: ToolsParams = serde_json::from_value(json!({"refresh": true})).expect("refresh");
    assert!(given.refresh);
}
