//! The legacy key `agents.default` is an alias of `agents.default_kind`: reads and writes resolve to the canonical key,
//! and a write keeps only the canonical row so the two can never disagree.

use std::path::Path;

use massorca_core_store::Store;
use massorca_protocol::methods::config::{ConfigKey, ConfigSet};
use serde_json::{Value, json};

use super::common::{connect, profile, start, stop, temp_home};

const CANONICAL: &str = "agents.default_kind";
const LEGACY: &str = "agents.default";

fn key(name: &str) -> ConfigKey {
    ConfigKey {
        owner: None,
        key: name.to_owned(),
    }
}

fn set(name: &str, value: Value) -> ConfigSet {
    ConfigSet {
        owner: None,
        key: name.to_owned(),
        value,
    }
}

/// Writes a legacy row into the profile's store while no service runs, as a profile saved before the rename holds it.
async fn seed_legacy(home: &Path, json_value: &str) {
    let first = start(home, "default").await;
    stop(first).await;
    let store = Store::open(profile(home, "default").store_path()).expect("the store opens");
    store
        .set_setting("core", LEGACY, json_value)
        .expect("the legacy row is written");
}

fn rows(home: &Path) -> (Option<String>, Option<String>) {
    let store = Store::open(profile(home, "default").store_path()).expect("the store opens");
    (
        store
            .get_setting("core", LEGACY)
            .expect("the legacy row reads"),
        store
            .get_setting("core", CANONICAL)
            .expect("the canonical row reads"),
    )
}

#[tokio::test]
async fn a_profile_with_only_the_legacy_key_reads_it_under_the_canonical_key() {
    let home = temp_home();
    seed_legacy(home.path(), "\"codex\"").await;
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let value = client
        .config_get(&key(CANONICAL))
        .await
        .expect("config.get answers");
    assert_eq!(value.key, CANONICAL);
    assert_eq!(value.value, json!("codex"));
    assert!(!value.is_default);

    let through_legacy = client
        .config_get(&key(LEGACY))
        .await
        .expect("config.get answers through the legacy name");
    assert_eq!(through_legacy.key, CANONICAL);
    assert_eq!(through_legacy.value, json!("codex"));
    stop(running).await;
}

#[tokio::test]
async fn writing_the_canonical_key_stores_only_the_canonical_row() {
    let home = temp_home();
    seed_legacy(home.path(), "\"codex\"").await;
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let stored = client
        .config_set(&set(CANONICAL, json!("gemini")))
        .await
        .expect("config.set answers");
    assert_eq!(stored.key, CANONICAL);
    assert_eq!(stored.value, json!("gemini"));
    stop(running).await;

    let (legacy, canonical) = rows(home.path());
    assert_eq!(legacy, None, "the legacy row is removed by the write");
    assert_eq!(canonical.as_deref(), Some("\"gemini\""));
}

#[tokio::test]
async fn writing_through_the_legacy_name_maps_to_the_canonical_key() {
    let home = temp_home();
    seed_legacy(home.path(), "\"codex\"").await;
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let stored = client
        .config_set(&set(LEGACY, json!("gemini")))
        .await
        .expect("config.set answers through the legacy name");
    assert_eq!(stored.key, CANONICAL, "the answer names the canonical key");
    assert_eq!(stored.value, json!("gemini"));
    let read = client
        .config_get(&key(CANONICAL))
        .await
        .expect("config.get answers");
    assert_eq!(read.value, json!("gemini"));
    stop(running).await;

    let (legacy, canonical) = rows(home.path());
    assert_eq!(legacy, None);
    assert_eq!(canonical.as_deref(), Some("\"gemini\""));
}

#[tokio::test]
async fn the_list_shows_the_canonical_key_only() {
    let home = temp_home();
    seed_legacy(home.path(), "\"codex\"").await;
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let list = client.config_list().await.expect("config.list answers");
    let names: Vec<&str> = list
        .values
        .iter()
        .filter(|entry| entry.key.starts_with("agents.default"))
        .map(|entry| entry.key.as_str())
        .collect();
    assert_eq!(names, [CANONICAL]);
    let entry = list
        .values
        .iter()
        .find(|entry| entry.key == CANONICAL)
        .expect("the canonical key is listed");
    assert_eq!(entry.value, json!("codex"));
    stop(running).await;
}

#[tokio::test]
async fn a_reset_removes_the_legacy_row_so_the_old_value_cannot_return() {
    let home = temp_home();
    seed_legacy(home.path(), "\"codex\"").await;
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let reset = client
        .config_reset(&key(CANONICAL))
        .await
        .expect("config.reset answers");
    assert!(reset.is_default);
    assert_eq!(reset.value, json!("claude"));
    stop(running).await;

    let (legacy, canonical) = rows(home.path());
    assert_eq!(legacy, None, "the legacy row is removed by the reset");
    assert_eq!(canonical, None);
}
