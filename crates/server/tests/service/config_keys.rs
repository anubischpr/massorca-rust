//! The core settings of decision 0070: their kinds on the wire, typed validation, reset, and `config.changed`.

use massorca_client::Subscription;
use massorca_protocol::codes;
use massorca_protocol::methods::config::{ConfigKey, ConfigSet};
use massorca_protocol::methods::events::{self, ChangeEvent};
use serde_json::{Value, json};
use tokio::time::timeout;

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

async fn next_event(subscription: &mut Subscription) -> ChangeEvent {
    timeout(
        super::common::REPLY_WAIT,
        subscription.next_as::<ChangeEvent>(),
    )
    .await
    .expect("an event arrives")
    .expect("the stream is open")
    .expect("the event decodes")
}

#[tokio::test]
async fn the_list_describes_each_kind() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let list = client.config_list().await.expect("config.list answers");
    assert_eq!(
        list.values.len(),
        38,
        "32 keys of 0070, terminal.theme, the first-prompt rename, the workspace root and layout, two branch prefix keys, usage.enabled"
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
    assert_eq!(depth.value, json!(1));

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
    let layout = find("workspaces.layout");
    assert_eq!(layout.kind.as_deref(), Some("choice"));
    assert_eq!(
        layout.choices.as_deref(),
        Some(
            &[
                "nested".to_owned(),
                "flat".to_owned(),
                "in_project".to_owned()
            ][..]
        )
    );
    assert_eq!(layout.value, json!("in_project"));
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

#[tokio::test]
async fn changes_and_resets_publish_config_changed_only_when_the_value_moves() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let mut subscription = client
        .subscribe_events()
        .await
        .expect("events.subscribe answers");
    let changed = |id: &str| ChangeEvent {
        topic: events::TOPIC_CONFIG_CHANGED.to_owned(),
        id: id.to_owned(),
    };

    // Writing the default that already applies changes nothing.
    client
        .config_set(&set("appearance.theme", json!("system")))
        .await
        .expect("config.set answers");
    client
        .config_set(&set("appearance.theme", json!("dark")))
        .await
        .expect("config.set answers");
    assert_eq!(
        next_event(&mut subscription).await,
        changed("core/appearance.theme")
    );

    client
        .config_set(&set("appearance.theme", json!("dark")))
        .await
        .expect("the same value again");
    let reset = client
        .config_reset(&key("appearance.theme"))
        .await
        .expect("config.reset answers");
    assert!(reset.is_default);
    assert_eq!(reset.value, json!("system"));
    assert_eq!(
        next_event(&mut subscription).await,
        changed("core/appearance.theme"),
        "the repeated set published nothing; this is the reset"
    );

    client
        .config_reset(&key("appearance.theme"))
        .await
        .expect("resetting a default succeeds");
    client
        .config_set(&set("editor.tab_width", json!(2)))
        .await
        .expect("config.set answers");
    assert_eq!(
        next_event(&mut subscription).await,
        changed("core/editor.tab_width"),
        "the second reset published nothing"
    );

    let unknown = client
        .config_reset(&key("no.such.key"))
        .await
        .expect_err("an unknown key cannot be reset");
    assert_eq!(remote_code(&unknown), codes::CONFIG_UNKNOWN_KEY);
    stop(running).await;
}
