//! The core settings of decision 0070: their kinds on the wire and typed validation.

use massorca_protocol::codes;
use massorca_protocol::methods::config::{ConfigKey, ConfigSet};
use serde_json::{Value, json};

use super::common::{connect, remote_code, start, stop, temp_home};

fn key(key: &str) -> ConfigKey {
    ConfigKey {
        owner: None,
        key: key.to_owned(),
    }
}

fn set(key: &str, value: Value) -> ConfigSet {
    ConfigSet {
        owner: None,
        key: key.to_owned(),
        value,
    }
}

#[tokio::test]
async fn the_list_describes_each_kind() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let list = client.config_list().await.expect("config.list answers");
    assert_eq!(
        list.values.len(),
        32,
        "31 keys of 0070 and the workspace root"
    );
    let find = |name: &str| {
        list.values
            .iter()
            .find(|entry| entry.key == name)
            .unwrap_or_else(|| panic!("{name} is listed"))
    };

    let theme = find("appearance.theme");
    assert_eq!(theme.kind.as_deref(), Some("choice"));
    assert_eq!(
        theme.choices.as_deref(),
        Some(&["system".to_owned(), "dark".to_owned(), "light".to_owned()][..])
    );
    assert_eq!(theme.value, json!("system"));

    let depth = find("agents.delegation_depth");
    assert_eq!(depth.kind.as_deref(), Some("integer"));
    assert_eq!((depth.min, depth.max), (Some(1), Some(5)));
    assert_eq!(depth.value, json!(3));

    assert_eq!(
        find("input.keybindings").kind.as_deref(),
        Some("keybindings")
    );
    assert_eq!(find("appearance.accent").kind.as_deref(), Some("colour"));
    assert_eq!(find("appearance.status_bar").kind.as_deref(), Some("set"));
    let folder = find("conversations.folder");
    assert_eq!(folder.kind.as_deref(), Some("path"));
    assert!(
        folder
            .value
            .as_str()
            .is_some_and(|path| path.ends_with("Conversations")),
        "{folder:?}"
    );
    assert_eq!(find("privacy.telemetry").value, json!(false));
    stop(running).await;
}

#[tokio::test]
async fn typed_values_are_checked() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    for (name, bad) in [
        ("appearance.theme", json!("purple")),
        ("agents.delegation_depth", json!(6)),
        ("agents.delegation_depth", json!(0)),
        ("appearance.text_size", json!(131)),
        ("appearance.accent", json!("#12345")),
        ("appearance.status_bar", json!(["memory", "memory"])),
        ("privacy.telemetry", json!("yes")),
    ] {
        let error = client
            .config_set(&set(name, bad.clone()))
            .await
            .expect_err("the value does not fit");
        assert_eq!(
            remote_code(&error),
            codes::CONFIG_INVALID_VALUE,
            "{name} {bad}"
        );
    }

    let conflict = client
        .config_set(&set(
            "input.keybindings",
            json!({ "app.quick_open": "Mod+P", "notes.pin": "Mod+P" }),
        ))
        .await
        .expect_err("two actions on one shortcut are refused");
    let message = conflict.to_string();
    assert!(
        message.contains("app.quick_open") && message.contains("notes.pin"),
        "{message}"
    );

    let moved = client
        .config_set(&set(
            "input.keybindings",
            json!({ "app.quick_open": null, "notes.pin": "Mod+P" }),
        ))
        .await
        .expect("a reassignment in one write is accepted");
    assert!(!moved.is_default);
    let accent = client
        .config_set(&set("appearance.accent", json!("#7461d8")))
        .await
        .expect("a custom colour is accepted");
    assert_eq!(accent.value, json!("#7461d8"));
    stop(running).await;
}
