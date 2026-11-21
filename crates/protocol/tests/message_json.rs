use massorca_protocol::{Auth, ErrorBody, Message, PeerInfo, ProtocolRange};
use serde::Deserialize;
use serde_json::{Value, json};

fn peer(name: &str) -> PeerInfo {
    PeerInfo {
        name: name.to_owned(),
        version: "0.1.0".to_owned(),
    }
}

fn to_json(message: &Message) -> Value {
    serde_json::to_value(message).expect("message serializes")
}

fn from_json(value: Value) -> Message {
    serde_json::from_value(value).expect("message deserializes")
}

fn hello_without_auth() -> Message {
    Message::Hello {
        protocol: ProtocolRange { min: 1, max: 1 },
        client: peer("massorca-cli"),
        capabilities: vec!["service/1".to_owned()],
        auth: None,
    }
}

#[test]
fn hello_has_the_documented_wire_shape() {
    assert_eq!(
        to_json(&hello_without_auth()),
        json!({
            "type": "hello",
            "protocol": { "min": 1, "max": 1 },
            "client": { "name": "massorca-cli", "version": "0.1.0" },
            "capabilities": ["service/1"]
        })
    );
}

#[test]
fn hello_carries_auth_only_when_present() {
    let with_auth = Message::Hello {
        protocol: ProtocolRange { min: 1, max: 1 },
        client: peer("massorca-cli"),
        capabilities: Vec::new(),
        auth: Some(Auth::from(json!({ "kind": "local" }))),
    };
    assert_eq!(to_json(&with_auth)["auth"], json!({ "kind": "local" }));
    assert!(to_json(&hello_without_auth()).get("auth").is_none());
}

#[test]
fn welcome_has_the_documented_wire_shape() {
    let message = Message::Welcome {
        protocol: 1,
        server: peer("massorca-service"),
        capabilities: vec!["service/1".to_owned(), "projects/1".to_owned()],
        connection_id: "conn-1".to_owned(),
        max_frame_len: None,
    };
    assert_eq!(
        to_json(&message),
        json!({
            "type": "welcome",
            "protocol": 1,
            "server": { "name": "massorca-service", "version": "0.1.0" },
            "capabilities": ["service/1", "projects/1"],
            "connection_id": "conn-1"
        })
    );
}

#[test]
fn request_has_the_documented_wire_shape() {
    let message = Message::Request {
        id: 7,
        method: "project.list".to_owned(),
        params: json!({}),
    };
    assert_eq!(
        to_json(&message),
        json!({ "type": "request", "id": 7, "method": "project.list", "params": {} })
    );
}

#[test]
fn response_ok_carries_result_and_no_error() {
    assert_eq!(
        to_json(&Message::response_ok(7, json!({ "projects": [] }))),
        json!({ "type": "response", "id": 7, "result": { "projects": [] } })
    );
}

#[test]
fn response_err_carries_error_and_no_result() {
    let message = Message::response_err(7, ErrorBody::new("project.not_found", "no such project"));
    assert_eq!(
        to_json(&message),
        json!({
            "type": "response",
            "id": 7,
            "error": { "code": "project.not_found", "message": "no such project" }
        })
    );
}

#[test]
fn event_has_the_documented_wire_shape() {
    let message = Message::Event {
        stream: 3,
        data: json!({ "topic": "project.added", "id": "p-1" }),
    };
    assert_eq!(
        to_json(&message),
        json!({
            "type": "event",
            "stream": 3,
            "data": { "topic": "project.added", "id": "p-1" }
        })
    );
}

#[test]
fn end_omits_error_when_the_stream_finished_cleanly() {
    assert_eq!(
        to_json(&Message::End {
            stream: 3,
            error: None
        }),
        json!({ "type": "end", "stream": 3 })
    );
}

#[test]
fn end_carries_error_when_the_stream_failed() {
    let message = Message::End {
        stream: 3,
        error: Some(ErrorBody::new("internal", "boom")),
    };
    assert_eq!(
        to_json(&message),
        json!({ "type": "end", "stream": 3, "error": { "code": "internal", "message": "boom" } })
    );
}

#[test]
fn cancel_has_the_documented_wire_shape() {
    assert_eq!(
        to_json(&Message::Cancel { stream: 3 }),
        json!({ "type": "cancel", "stream": 3 })
    );
}

#[test]
fn every_variant_round_trips_through_json() {
    let messages = [
        Message::Hello {
            protocol: ProtocolRange { min: 1, max: 2 },
            client: peer("massorca-cli"),
            capabilities: vec!["events/1".to_owned()],
            auth: Some(Auth::from(json!({ "kind": "local" }))),
        },
        Message::Welcome {
            protocol: 1,
            server: peer("massorca-service"),
            capabilities: vec!["service/1".to_owned()],
            connection_id: "conn-1".to_owned(),
            max_frame_len: Some(1_048_576),
        },
        Message::Request {
            id: 7,
            method: "workspace.list".to_owned(),
            params: json!({ "project": "app" }),
        },
        Message::response_ok(7, json!({ "workspaces": [] })),
        Message::response_ok(8, Value::Null),
        Message::response_err(
            9,
            ErrorBody {
                code: "internal".to_owned(),
                message: "boom".to_owned(),
                data: Some(json!({ "hint": "retry" })),
            },
        ),
        Message::Event {
            stream: 3,
            data: json!({ "topic": "workspace.removed", "id": "w-1" }),
        },
        Message::End {
            stream: 3,
            error: None,
        },
        Message::End {
            stream: 3,
            error: Some(ErrorBody::new("stream.not_found", "gone")),
        },
        Message::Cancel { stream: 3 },
        Message::Unknown,
    ];
    for message in messages {
        assert_eq!(from_json(to_json(&message)), message);
    }
}

#[test]
fn unknown_type_decodes_to_unknown() {
    assert_eq!(
        from_json(json!({ "type": "future_message", "payload": [1, 2] })),
        Message::Unknown
    );
}

#[test]
fn unknown_fields_are_ignored_at_every_level() {
    assert_eq!(
        from_json(json!({ "type": "cancel", "stream": 3, "extra": true })),
        Message::Cancel { stream: 3 }
    );
    assert_eq!(
        from_json(json!({
            "type": "hello",
            "protocol": { "min": 1, "max": 1, "extra": 1 },
            "client": { "name": "massorca-cli", "version": "0.1.0", "extra": 2 },
            "capabilities": [],
            "trace": "x"
        })),
        Message::Hello {
            protocol: ProtocolRange { min: 1, max: 1 },
            client: peer("massorca-cli"),
            capabilities: Vec::new(),
            auth: None,
        }
    );
}

#[test]
fn none_fields_are_omitted_not_written_as_null() {
    let end = to_json(&Message::End {
        stream: 1,
        error: None,
    });
    assert!(end.get("error").is_none());
    let body = serde_json::to_value(ErrorBody::new("internal", "boom")).expect("serializes");
    assert!(body.get("data").is_none());
}

#[test]
fn missing_optional_fields_decode_as_none_or_empty() {
    assert_eq!(
        from_json(json!({
            "type": "hello",
            "protocol": { "min": 1, "max": 1 },
            "client": { "name": "massorca-cli", "version": "0.1.0" }
        })),
        Message::Hello {
            protocol: ProtocolRange { min: 1, max: 1 },
            client: peer("massorca-cli"),
            capabilities: Vec::new(),
            auth: None,
        }
    );
    assert_eq!(
        from_json(json!({ "type": "end", "stream": 4 })),
        Message::End {
            stream: 4,
            error: None
        }
    );
}

#[test]
fn null_result_is_a_present_result() {
    assert_eq!(
        from_json(json!({ "type": "response", "id": 9, "result": null })),
        Message::response_ok(9, Value::Null)
    );
}

#[test]
fn request_params_default_to_null_when_absent() {
    assert_eq!(
        from_json(json!({ "type": "request", "id": 1, "method": "service.info" })),
        Message::Request {
            id: 1,
            method: "service.info".to_owned(),
            params: Value::Null,
        }
    );
}

#[test]
fn value_without_type_is_rejected() {
    assert!(serde_json::from_value::<Message>(json!({ "id": 1 })).is_err());
}

#[test]
fn response_constructors_set_exactly_one_of_result_or_error() {
    let Message::Response { result, error, .. } = Message::response_ok(1, json!(true)) else {
        panic!("response_ok builds a response");
    };
    assert!(result.is_some());
    assert!(error.is_none());

    let Message::Response { result, error, .. } =
        Message::response_err(1, ErrorBody::new("internal", "boom"))
    else {
        panic!("response_err builds a response");
    };
    assert!(result.is_none());
    assert!(error.is_some());
}

#[test]
fn error_body_new_leaves_data_unset() {
    let body = ErrorBody::new("project.not_found", "no such project");
    assert_eq!(body.code, "project.not_found");
    assert_eq!(body.message, "no such project");
    assert!(body.data.is_none());
}

#[test]
fn rejected_has_the_documented_wire_shape() {
    let message = Message::Rejected {
        error: ErrorBody::new("protocol.unsupported_version", "no common version"),
        supported: ProtocolRange { min: 1, max: 2 },
    };
    let wire = json!({
        "type": "rejected",
        "error": { "code": "protocol.unsupported_version", "message": "no common version" },
        "supported": { "min": 1, "max": 2 }
    });
    assert_eq!(to_json(&message), wire);
    assert_eq!(from_json(wire), message);
}

/// The message set of a build that predates `rejected`.
#[derive(Debug, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OldMessage {
    Welcome {},
    #[serde(other)]
    Unknown,
}

#[test]
fn an_older_receiver_decodes_rejected_as_unknown() {
    let rejected = to_json(&Message::Rejected {
        error: ErrorBody::new("protocol.handshake", "malformed hello"),
        supported: ProtocolRange { min: 1, max: 1 },
    });
    let decoded: OldMessage = serde_json::from_value(rejected).expect("old receiver decodes");
    assert_eq!(decoded, OldMessage::Unknown);
}

#[test]
fn response_with_both_result_and_error_fails_to_decode() {
    let both = json!({
        "type": "response",
        "id": 1,
        "result": true,
        "error": { "code": "internal", "message": "boom" }
    });
    assert!(serde_json::from_value::<Message>(both).is_err());
}

#[test]
fn response_with_neither_result_nor_error_fails_to_decode() {
    let neither = json!({ "type": "response", "id": 1 });
    assert!(serde_json::from_value::<Message>(neither).is_err());
}

#[test]
fn welcome_carries_max_frame_len_only_when_set() {
    let welcome = |max_frame_len| Message::Welcome {
        protocol: 1,
        server: peer("massorca-service"),
        capabilities: Vec::new(),
        connection_id: "conn-1".to_owned(),
        max_frame_len,
    };
    let limited = to_json(&welcome(Some(1_048_576)));
    assert_eq!(limited["max_frame_len"], json!(1_048_576));
    assert_eq!(from_json(limited), welcome(Some(1_048_576)));

    let default = to_json(&welcome(None));
    assert!(default.get("max_frame_len").is_none());
    assert_eq!(from_json(default), welcome(None));
}

#[test]
fn hello_debug_redacts_auth() {
    let hello = Message::Hello {
        protocol: ProtocolRange { min: 1, max: 1 },
        client: peer("massorca-cli"),
        capabilities: Vec::new(),
        auth: Some(Auth::from(json!({ "token": "s3cret-token" }))),
    };
    let printed = format!("{hello:?}");
    assert!(!printed.contains("s3cret-token"), "{printed}");
    assert!(printed.contains("<redacted>"), "{printed}");
    assert_eq!(to_json(&hello)["auth"], json!({ "token": "s3cret-token" }));
}
