//! The wire of a kind's standing launch settings (decision 0174, capability `agent-settings/1`).

use std::collections::BTreeMap;

use massorca_protocol::capabilities;
use massorca_protocol::methods::agents::{
    self, KindSettings, KindSettingsCleared, KindSettingsRef, KindSettingsSetParams,
};
use serde_json::json;

#[test]
fn the_method_names_and_the_capability_match_the_wire_table() {
    assert_eq!(agents::SETTINGS_GET, "agent.settings.get");
    assert_eq!(agents::SETTINGS_SET, "agent.settings.set");
    assert_eq!(agents::SETTINGS_CLEAR, "agent.settings.clear");
    assert_eq!(capabilities::AGENT_SETTINGS, "agent-settings/1");
}

#[test]
fn set_params_name_only_what_they_change() {
    let bare: KindSettingsSetParams =
        serde_json::from_value(json!({ "kind": "claude" })).expect("decodes");
    assert_eq!(
        bare,
        KindSettingsSetParams {
            kind: "claude".to_owned(),
            ..Default::default()
        }
    );
    assert_eq!(
        serde_json::to_value(&bare).expect("encodes"),
        json!({ "kind": "claude" })
    );

    let all = KindSettingsSetParams {
        kind: "claude".to_owned(),
        program: Some("/opt/agents/claude-wrapper".to_owned()),
        args: Some(vec!["--verbose".to_owned()]),
        env: BTreeMap::from([("PROXY_REGION".to_owned(), "placeholder-eu".to_owned())]),
        remove_env: vec!["OLD_NAME".to_owned()],
    };
    let wire = json!({
        "kind": "claude",
        "program": "/opt/agents/claude-wrapper",
        "args": ["--verbose"],
        "env": { "PROXY_REGION": "placeholder-eu" },
        "remove_env": ["OLD_NAME"]
    });
    assert_eq!(serde_json::to_value(&all).expect("encodes"), wire);
    let decoded: KindSettingsSetParams = serde_json::from_value(wire).expect("decodes");
    assert_eq!(decoded, all);

    // An empty list is sent, so a caller can clear the arguments.
    let no_args = KindSettingsSetParams {
        kind: "claude".to_owned(),
        args: Some(Vec::new()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(&no_args).expect("encodes"),
        json!({ "kind": "claude", "args": [] })
    );
}

#[test]
fn set_params_never_print_an_env_value() {
    let params = KindSettingsSetParams {
        kind: "claude".to_owned(),
        env: BTreeMap::from([("API_TOKEN".to_owned(), "placeholder-value-5d0e".to_owned())]),
        ..Default::default()
    };
    let shown = format!("{params:?}");
    assert!(shown.contains("API_TOKEN"), "{shown}");
    assert!(!shown.contains("placeholder-value-5d0e"), "{shown}");
}

#[test]
fn settings_carry_env_names_and_never_values() {
    let settings = KindSettings {
        kind: "codex".to_owned(),
        program: None,
        args: vec!["-c".to_owned(), "x=1".to_owned()],
        env_names: vec!["PROXY_REGION".to_owned()],
    };
    let wire = json!({ "kind": "codex", "args": ["-c", "x=1"], "env_names": ["PROXY_REGION"] });
    assert_eq!(serde_json::to_value(&settings).expect("encodes"), wire);
    let decoded: KindSettings = serde_json::from_value(wire).expect("decodes");
    assert_eq!(decoded, settings);

    let empty: KindSettings = serde_json::from_value(json!({ "kind": "codex" })).expect("decodes");
    assert!(empty.args.is_empty() && empty.env_names.is_empty() && empty.program.is_none());
}

#[test]
fn a_kind_ref_and_a_clear_result_are_plain() {
    let reference = KindSettingsRef {
        kind: "pi".to_owned(),
    };
    assert_eq!(
        serde_json::to_value(&reference).expect("encodes"),
        json!({ "kind": "pi" })
    );
    let cleared = KindSettingsCleared {
        kind: "pi".to_owned(),
        removed: true,
    };
    assert_eq!(
        serde_json::to_value(&cleared).expect("encodes"),
        json!({ "kind": "pi", "removed": true })
    );
}
