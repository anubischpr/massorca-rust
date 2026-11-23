//! Settings: the declared default, validation, storage under the core owner, and persistence across restarts.

use std::path::Path;

use massorca_protocol::codes;
use massorca_protocol::methods::config::{self, ConfigKey, ConfigSet, ConfigValue};
use massorca_server::{Owner, SettingDecl, SettingKind, SettingsRegistry};
use serde_json::{Value, json};

use super::common::{connect, remote_code, start, stop, temp_home};

const WORKSPACES_ROOT: &str = "workspaces.default_root";

fn key(owner: Option<&str>) -> ConfigKey {
    ConfigKey {
        owner: owner.map(str::to_owned),
        key: WORKSPACES_ROOT.to_owned(),
    }
}

fn set(value: Value) -> ConfigSet {
    ConfigSet {
        owner: None,
        key: WORKSPACES_ROOT.to_owned(),
        value,
    }
}

#[tokio::test]
async fn the_default_workspace_root_is_declared_and_listed() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let value = client
        .config_get(&key(None))
        .await
        .expect("config.get answers");
    assert_eq!(value.owner, "core");
    assert_eq!(value.key, WORKSPACES_ROOT);
    assert!(value.is_default);
    assert_eq!(value.value, value.default);
    let root = value
        .default
        .as_str()
        .expect("the default is a path string");
    assert!(Path::new(root).is_absolute(), "{root} is absolute");
    assert!(
        Path::new(root).ends_with("massorca/workspaces"),
        "{root} ends in massorca/workspaces"
    );

    let listed = client.config_list().await.expect("config.list answers");
    assert!(
        listed
            .values
            .iter()
            .any(|entry| entry.owner == "core" && entry.key == WORKSPACES_ROOT && entry.is_default)
    );
    stop(running).await;
}

#[tokio::test]
async fn a_valid_value_is_stored_under_core_and_read_back() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let chosen = home
        .path()
        .join("chosen-workspaces")
        .to_str()
        .expect("temporary paths are UTF-8")
        .to_owned();

    let stored = client
        .config_set(&set(json!(chosen)))
        .await
        .expect("config.set answers");
    assert_eq!(stored.owner, "core", "the owner defaults to core");
    assert!(!stored.is_default);
    assert_eq!(stored.value, json!(chosen));

    let read = client
        .config_get(&key(Some("core")))
        .await
        .expect("config.get answers");
    assert_eq!(read.value, json!(chosen));
    let listed = client.config_list().await.expect("config.list answers");
    assert!(
        listed
            .values
            .iter()
            .any(|entry| entry.key == WORKSPACES_ROOT && entry.value == json!(chosen))
    );
    stop(running).await;
}

#[tokio::test]
async fn invalid_values_are_refused_and_leave_the_setting_as_it_was() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let before = client
        .config_get(&key(None))
        .await
        .expect("config.get answers");

    for bad in [
        json!("relative/workspaces"),
        json!(""),
        json!(42),
        json!(true),
    ] {
        let error = client
            .config_set(&set(bad.clone()))
            .await
            .expect_err("the value is not a usable path");
        assert_eq!(
            remote_code(&error),
            codes::CONFIG_INVALID_VALUE,
            "value {bad}"
        );
    }

    let after = client
        .config_get(&key(None))
        .await
        .expect("config.get answers");
    assert_eq!(after.value, before.value);
    assert!(after.is_default);
    stop(running).await;
}

#[tokio::test]
async fn unknown_keys_and_owners_are_refused() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let unknown_key = client
        .config_get(&ConfigKey {
            owner: None,
            key: "no.such.setting".to_owned(),
        })
        .await
        .expect_err("the key is not declared");
    assert_eq!(remote_code(&unknown_key), codes::CONFIG_UNKNOWN_KEY);

    let unknown_set = client
        .config_set(&ConfigSet {
            owner: None,
            key: "no.such.setting".to_owned(),
            value: json!(1),
        })
        .await
        .expect_err("the key is not declared");
    assert_eq!(remote_code(&unknown_set), codes::CONFIG_UNKNOWN_KEY);

    let other_owner = client
        .config_get(&key(Some("ext:demo")))
        .await
        .expect_err("core declares this key, not ext:demo");
    assert_eq!(remote_code(&other_owner), codes::CONFIG_UNKNOWN_KEY);

    let bogus_owner = client
        .config_get(&key(Some("someone")))
        .await
        .expect_err("someone is not an owner");
    assert_eq!(remote_code(&bogus_owner), codes::CONFIG_UNKNOWN_KEY);
    stop(running).await;
}

#[tokio::test]
async fn a_stored_value_survives_a_restart() {
    let home = temp_home();
    let chosen = home
        .path()
        .join("kept-workspaces")
        .to_str()
        .expect("temporary paths are UTF-8")
        .to_owned();

    let first = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    client
        .config_set(&set(json!(chosen)))
        .await
        .expect("config.set answers");
    client
        .shutdown()
        .await
        .expect("service.shutdown is answered");
    first.wait().await.expect("the first service stops cleanly");

    let second = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let read = client
        .config_get(&key(None))
        .await
        .expect("config.get answers after the restart");
    assert_eq!(read.value, json!(chosen));
    stop(second).await;
}

#[tokio::test]
async fn each_owner_reads_its_own_value_for_a_shared_key() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let _extension = running
        .registries()
        .settings
        .register(SettingDecl {
            owner: Owner::Extension("demo".to_owned()),
            key: WORKSPACES_ROOT.to_owned(),
            kind: SettingKind::String,
            default: Value::String("from demo".to_owned()),
            description: "declared by the demo extension".to_owned(),
            aliases: Vec::new(),
        })
        .expect("the extension declares the key");
    let client = connect(home.path(), "default").await;

    let demo: ConfigValue = client
        .call(config::GET, &key(Some("ext:demo")))
        .await
        .expect("the extension's setting is readable");
    assert_eq!(demo.owner, "ext:demo");
    assert_eq!(demo.value, Value::String("from demo".to_owned()));

    // Core declares the same key with its own default; reading it must not return the extension's value.
    let core: ConfigValue = client
        .call(config::GET, &key(None))
        .await
        .expect("core's setting is readable");
    assert_eq!(core.owner, "core");
    assert_ne!(core.value, Value::String("from demo".to_owned()));
    stop(running).await;
}

/// Owner ids and setting keys use lower-case letters, digits, '.', '_' and '-', so the separator the registry joins them
/// with can never occur in either of them.
#[test]
fn owner_ids_and_keys_outside_the_name_rule_are_refused_at_declaration() {
    let settings = SettingsRegistry::default();
    let declare = |owner: Owner, key: &str| {
        settings.register(SettingDecl {
            owner,
            aliases: Vec::new(),
            key: key.to_owned(),
            kind: SettingKind::String,
            default: json!("x"),
            description: "a test setting".to_owned(),
        })
    };
    let demo = || Owner::Extension("demo".to_owned());

    assert!(
        declare(Owner::Extension("a\u{1f}b".to_owned()), "key").is_err(),
        "a separator in an owner id is refused"
    );
    assert!(
        declare(Owner::Extension("Demo".to_owned()), "key").is_err(),
        "an upper-case owner id is refused"
    );
    assert!(
        declare(demo(), "bad\u{1f}key").is_err(),
        "a separator in a key is refused"
    );
    assert!(
        declare(demo(), "Key").is_err(),
        "an upper-case key is refused"
    );
    assert!(
        declare(demo(), "good.key_1-x").is_ok(),
        "a key within the rule is accepted"
    );
}

#[tokio::test]
async fn config_refuses_owners_and_keys_outside_the_name_rule() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let bad_owner = client
        .config_get(&key(Some("ext:a\u{1f}b")))
        .await
        .expect_err("no owner can carry that id");
    assert_eq!(remote_code(&bad_owner), codes::CONFIG_UNKNOWN_KEY);

    let bad_key = client
        .config_get(&ConfigKey {
            owner: None,
            key: "workspaces\u{1f}default_root".to_owned(),
        })
        .await
        .expect_err("no setting has that key");
    assert_eq!(remote_code(&bad_key), codes::CONFIG_UNKNOWN_KEY);
    stop(running).await;
}
