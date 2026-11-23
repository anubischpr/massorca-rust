//! The `devices` table of decision 0041.

use massorca_core_store::{DeviceRow, Store, StoreError, new_id};

fn device(name: &str, key_pin: &str) -> DeviceRow {
    DeviceRow {
        id: new_id(),
        name: name.to_owned(),
        key_pin: key_pin.to_owned(),
        scopes: vec!["read".to_owned(), "workspace".to_owned()],
        paired_at: 10,
        last_seen_at: None,
        revoked_at: None,
    }
}

#[test]
fn devices_are_found_by_key_and_listed_in_pairing_order() {
    let store = Store::open_in_memory().unwrap();
    let laptop = device("laptop", "sha256:a");
    let mut phone = device("phone", "sha256:b");
    phone.paired_at = 5;
    store.insert_device(&laptop).unwrap();
    store.insert_device(&phone).unwrap();
    assert_eq!(
        store.find_device_by_key_pin("sha256:a").unwrap(),
        Some(laptop.clone())
    );
    assert_eq!(store.find_device_by_key_pin("sha256:z").unwrap(), None);
    let names: Vec<_> = store
        .list_devices()
        .unwrap()
        .into_iter()
        .map(|row| row.name)
        .collect();
    assert_eq!(names, ["phone", "laptop"]);
}

#[test]
fn a_key_pairs_once() {
    let store = Store::open_in_memory().unwrap();
    store.insert_device(&device("a", "sha256:same")).unwrap();
    let again = store.insert_device(&device("b", "sha256:same"));
    assert!(matches!(again, Err(StoreError::Conflict { .. })));
}

#[test]
fn revoking_is_recorded_once_and_touch_updates_last_seen() {
    let store = Store::open_in_memory().unwrap();
    let row = device("laptop", "sha256:a");
    store.insert_device(&row).unwrap();
    store.touch_device(&row.id, 20).unwrap();
    assert!(store.revoke_device(&row.id, 30).unwrap());
    assert!(!store.revoke_device(&row.id, 40).unwrap());
    assert!(!store.revoke_device("missing", 40).unwrap());
    let stored = store.get_device(&row.id).unwrap().unwrap();
    assert_eq!(stored.last_seen_at, Some(20));
    assert_eq!(stored.revoked_at, Some(30));
    assert_eq!(stored.scopes, ["read", "workspace"]);
}

#[test]
fn a_grant_widens_an_active_device_and_leaves_a_revoked_one() {
    let store = Store::open_in_memory().unwrap();
    let active = device("laptop", "sha256:a");
    let revoked = device("phone", "sha256:b");
    store.insert_device(&active).unwrap();
    store.insert_device(&revoked).unwrap();
    store.revoke_device(&revoked.id, 30).unwrap();
    let all: Vec<String> = ["read", "workspace", "host-admin", "device-admin"]
        .map(str::to_owned)
        .to_vec();

    assert!(store.grant_device_scopes(&active.id, &all).unwrap());
    assert!(!store.grant_device_scopes(&revoked.id, &all).unwrap());
    assert!(!store.grant_device_scopes("missing", &all).unwrap());
    assert_eq!(store.get_device(&active.id).unwrap().unwrap().scopes, all);
    assert_eq!(
        store.get_device(&revoked.id).unwrap().unwrap().scopes,
        ["read", "workspace"]
    );
}
