use massorca_core_store::Store;

fn store() -> Store {
    Store::open_in_memory().expect("in-memory store opens")
}

fn entry(owner: &str, key: &str, json: &str) -> (String, String, String) {
    (owner.to_owned(), key.to_owned(), json.to_owned())
}

#[test]
fn setting_round_trips_json_text_verbatim() {
    let store = store();
    let json = r#"{"theme":"dark","fontSize":13,"tabs":[1,2]}"#;
    store.set_setting("app", "layout", json).unwrap();

    assert_eq!(
        store.get_setting("app", "layout").unwrap(),
        Some(json.to_owned())
    );
    assert_eq!(store.get_setting("app", "missing").unwrap(), None);
}

#[test]
fn set_replaces_the_value_for_the_same_owner_and_key() {
    let store = store();
    store.set_setting("app", "theme", "\"light\"").unwrap();
    store.set_setting("app", "theme", "\"dark\"").unwrap();

    assert_eq!(
        store.get_setting("app", "theme").unwrap(),
        Some("\"dark\"".to_owned())
    );
    assert_eq!(store.list_settings(Some("app")).unwrap().len(), 1);
}

#[test]
fn settings_are_isolated_per_owner() {
    let store = store();
    store.set_setting("alpha", "enabled", "true").unwrap();
    store.set_setting("beta", "enabled", "false").unwrap();

    assert_eq!(
        store.get_setting("alpha", "enabled").unwrap(),
        Some("true".to_owned())
    );
    assert_eq!(
        store.get_setting("beta", "enabled").unwrap(),
        Some("false".to_owned())
    );
    assert_eq!(store.get_setting("gamma", "enabled").unwrap(), None);
}

#[test]
fn delete_reports_whether_a_setting_existed() {
    let store = store();
    store.set_setting("app", "theme", "\"dark\"").unwrap();

    assert!(store.delete_setting("app", "theme").unwrap());
    assert_eq!(store.get_setting("app", "theme").unwrap(), None);
    assert!(!store.delete_setting("app", "theme").unwrap());
}

#[test]
fn list_filters_by_owner_and_orders_by_owner_then_key() {
    let store = store();
    store.set_setting("beta", "b-key", "2").unwrap();
    store.set_setting("alpha", "z-key", "3").unwrap();
    store.set_setting("alpha", "a-key", "1").unwrap();

    assert_eq!(
        store.list_settings(Some("alpha")).unwrap(),
        vec![entry("alpha", "a-key", "1"), entry("alpha", "z-key", "3")]
    );
    assert_eq!(
        store.list_settings(None).unwrap(),
        vec![
            entry("alpha", "a-key", "1"),
            entry("alpha", "z-key", "3"),
            entry("beta", "b-key", "2"),
        ]
    );
}

#[test]
fn settings_survive_reopening_a_file_store() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite3");
    Store::open(&path)
        .unwrap()
        .set_setting("app", "theme", "\"dark\"")
        .unwrap();

    let reopened = Store::open(&path).unwrap();
    assert_eq!(
        reopened.get_setting("app", "theme").unwrap(),
        Some("\"dark\"".to_owned())
    );
}
