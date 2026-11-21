use massorca_protocol::methods::{config, events, service};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::fmt::Debug;

/// Checks that `value` serializes to `expected` and that `expected` decodes back to `value`.
fn assert_wire<T>(value: &T, expected: &Value)
where
    T: Serialize + DeserializeOwned + Debug + PartialEq,
{
    assert_eq!(&serde_json::to_value(value).expect("serializes"), expected);
    let decoded: T = serde_json::from_value(expected.clone()).expect("deserializes");
    assert_eq!(&decoded, value);
}

fn sample_config_value() -> config::ConfigValue {
    config::ConfigValue {
        owner: "core".to_owned(),
        key: "ui.zoom".to_owned(),
        value: json!(2),
        default: json!(1),
        is_default: false,
    }
}

fn sample_config_value_json() -> Value {
    json!({ "owner": "core", "key": "ui.zoom", "value": 2, "default": 1, "is_default": false })
}

#[test]
fn service_info_has_the_documented_shape() {
    let info = service::ServiceInfo {
        version: "0.1.0".to_owned(),
        protocol: 1,
        capabilities: vec!["service/1".to_owned()],
        profile: "default".to_owned(),
        pid: 4242,
        started_at_ms: 1_767_225_600_000_i64,
    };
    assert_wire(
        &info,
        &json!({
            "version": "0.1.0",
            "protocol": 1,
            "capabilities": ["service/1"],
            "profile": "default",
            "pid": 4242,
            "started_at_ms": 1_767_225_600_000_i64
        }),
    );
}

#[test]
fn subscribe_result_carries_the_stream_id() {
    assert_wire(&events::Subscribed { stream: 5 }, &json!({ "stream": 5 }));
}

#[test]
fn change_event_has_topic_and_id() {
    assert_wire(
        &events::ChangeEvent {
            topic: events::TOPIC_WORKSPACE_REMOVING.to_owned(),
            id: "w-1".to_owned(),
        },
        &json!({ "topic": "workspace.removing", "id": "w-1" }),
    );
}

#[test]
fn topic_constants_match_the_contract() {
    assert_eq!(events::TOPIC_WORKSPACE_CREATED, "workspace.created");
    assert_eq!(events::TOPIC_WORKSPACE_REMOVING, "workspace.removing");
    assert_eq!(events::TOPIC_WORKSPACE_REMOVED, "workspace.removed");
    assert_eq!(events::TOPIC_PROJECT_ADDED, "project.added");
    assert_eq!(events::TOPIC_PROJECT_REMOVED, "project.removed");
}

#[test]
fn config_key_omits_owner_when_unset() {
    assert_wire(
        &config::ConfigKey {
            owner: None,
            key: "ui.zoom".to_owned(),
        },
        &json!({ "key": "ui.zoom" }),
    );
}

#[test]
fn config_key_carries_owner_when_set() {
    assert_wire(
        &config::ConfigKey {
            owner: Some("tools".to_owned()),
            key: "ui.zoom".to_owned(),
        },
        &json!({ "owner": "tools", "key": "ui.zoom" }),
    );
}

#[test]
fn config_set_carries_any_json_value_including_null() {
    assert_wire(
        &config::ConfigSet {
            owner: None,
            key: "ui.zoom".to_owned(),
            value: json!(2),
        },
        &json!({ "key": "ui.zoom", "value": 2 }),
    );
    assert_wire(
        &config::ConfigSet {
            owner: Some("tools".to_owned()),
            key: "ui.zoom".to_owned(),
            value: Value::Null,
        },
        &json!({ "owner": "tools", "key": "ui.zoom", "value": null }),
    );
}

#[test]
fn config_value_has_the_documented_shape() {
    assert_wire(&sample_config_value(), &sample_config_value_json());
}

#[test]
fn config_list_has_the_documented_shape() {
    assert_wire(
        &config::ConfigList {
            values: vec![sample_config_value()],
        },
        &json!({ "values": [sample_config_value_json()] }),
    );
}

#[test]
fn config_default_owner_is_core() {
    assert_eq!(config::DEFAULT_OWNER, "core");
}
