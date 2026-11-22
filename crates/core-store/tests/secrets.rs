//! Vault rows: owned secrets keep only sealed bytes, links cascade with their secret, names are unique per scope, and
//! requests resolve once.

use massorca_core_store::{
    SecretKeyCheck, SecretLinkRow, SecretRequestRow, SecretRow, Store, StoreError,
};

fn store() -> Store {
    Store::open_in_memory().expect("in-memory store opens")
}

fn secret(id: &str, scope: &str, name: &str) -> SecretRow {
    SecretRow {
        id: id.to_owned(),
        scope: scope.to_owned(),
        name: name.to_owned(),
        nonce: vec![1; 24],
        ciphertext: vec![2; 40],
        created_at: 10,
        rotated_at: 10,
        used_at: None,
    }
}

fn link(scope: &str, name: &str, secret_id: &str) -> SecretLinkRow {
    SecretLinkRow {
        scope: scope.to_owned(),
        name: name.to_owned(),
        secret_id: secret_id.to_owned(),
        linked_at: 20,
    }
}

fn request(id: &str, scope: &str, name: &str, created_at: i64) -> SecretRequestRow {
    SecretRequestRow {
        id: id.to_owned(),
        scope: scope.to_owned(),
        name: name.to_owned(),
        reason: "Needed to call the API.".to_owned(),
        session: Some("session-1".to_owned()),
        requester: Some("owner".to_owned()),
        state: "pending".to_owned(),
        created_at,
        resolved_at: None,
    }
}

#[test]
fn a_secret_round_trips_and_rotates_its_sealed_bytes() {
    let store = store();
    let row = secret("s1", "project:p1", "OPENAI_API_KEY");
    store.insert_secret(&row).unwrap();
    assert_eq!(
        store.find_secret("project:p1", "OPENAI_API_KEY").unwrap(),
        Some(row.clone())
    );
    assert_eq!(store.secret_by_id("s1").unwrap(), Some(row));

    assert!(
        store
            .update_secret_value("s1", &[3; 24], &[4; 41], 30)
            .unwrap()
    );
    store.touch_secret("s1", 40).unwrap();
    let rotated = store.secret_by_id("s1").unwrap().unwrap();
    assert_eq!(
        (
            rotated.nonce,
            rotated.ciphertext,
            rotated.rotated_at,
            rotated.used_at
        ),
        (vec![3; 24], vec![4; 41], 30, Some(40))
    );
    assert!(!store.update_secret_value("missing", &[0], &[0], 1).unwrap());

    let listed = store.list_secrets().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "OPENAI_API_KEY");
}

#[test]
fn a_name_is_owned_once_per_scope() {
    let store = store();
    store
        .insert_secret(&secret("s1", "global", "TOKEN"))
        .unwrap();
    store
        .insert_secret(&secret("s2", "project:p1", "TOKEN"))
        .unwrap();
    let error = store
        .insert_secret(&secret("s3", "global", "TOKEN"))
        .unwrap_err();
    assert!(matches!(error, StoreError::Conflict { .. }), "{error:?}");
}

#[test]
fn deleting_a_secret_removes_its_links() {
    let store = store();
    store
        .insert_secret(&secret("s1", "project:p1", "KEY"))
        .unwrap();
    store
        .insert_secret_link(&link("project:p2", "KEY", "s1"))
        .unwrap();
    store
        .insert_secret_link(&link("team:t1", "KEY", "s1"))
        .unwrap();
    assert_eq!(
        store.find_secret_link("team:t1", "KEY").unwrap(),
        Some(link("team:t1", "KEY", "s1"))
    );
    assert!(matches!(
        store
            .insert_secret_link(&link("team:t1", "KEY", "s1"))
            .unwrap_err(),
        StoreError::Conflict { .. }
    ));
    assert!(matches!(
        store
            .insert_secret_link(&link("global", "KEY", "nope"))
            .unwrap_err(),
        StoreError::Constraint { .. }
    ));

    assert!(store.delete_secret_link("project:p2", "KEY").unwrap());
    assert!(!store.delete_secret_link("project:p2", "KEY").unwrap());
    assert!(store.delete_secret("s1").unwrap());
    assert_eq!(store.list_secret_links().unwrap(), Vec::new());
    assert!(!store.delete_secret("s1").unwrap());
}

#[test]
fn requests_resolve_once_and_by_name() {
    let store = store();
    store
        .insert_secret_request(&request("r1", "project:p1", "KEY", 1))
        .unwrap();
    store
        .insert_secret_request(&request("r2", "project:p1", "KEY", 2))
        .unwrap();
    store
        .insert_secret_request(&request("r3", "global", "OTHER", 3))
        .unwrap();

    assert!(store.resolve_secret_request("r3", "declined", 5).unwrap());
    assert!(
        !store.resolve_secret_request("r3", "stored", 6).unwrap(),
        "already resolved"
    );
    let mut ids = store
        .resolve_secret_requests_for("project:p1", "KEY", "stored", 7)
        .unwrap();
    ids.sort();
    assert_eq!(ids, ["r1", "r2"]);

    assert_eq!(store.list_secret_requests(true).unwrap(), Vec::new());
    let all = store.list_secret_requests(false).unwrap();
    let states: Vec<(&str, &str)> = all
        .iter()
        .map(|r| (r.id.as_str(), r.state.as_str()))
        .collect();
    assert_eq!(
        states,
        [("r1", "stored"), ("r2", "stored"), ("r3", "declined")]
    );
    assert_eq!(
        store.secret_request("r1").unwrap().unwrap().resolved_at,
        Some(7)
    );

    let mut bad = request("r4", "global", "KEY", 4);
    bad.state = "granted".to_owned();
    assert!(matches!(
        store.insert_secret_request(&bad).unwrap_err(),
        StoreError::Constraint { .. }
    ));
}

#[test]
fn the_key_check_is_one_row_that_can_be_replaced_and_cleared() {
    let store = store();
    assert_eq!(store.secret_key_check().unwrap(), None);
    let first = SecretKeyCheck {
        nonce: vec![1; 24],
        sealed: vec![2; 48],
    };
    store.set_secret_key_check(&first, 10).unwrap();
    assert_eq!(store.secret_key_check().unwrap(), Some(first));
    let second = SecretKeyCheck {
        nonce: vec![3; 24],
        sealed: vec![4; 48],
    };
    store.set_secret_key_check(&second, 20).unwrap();
    assert_eq!(store.secret_key_check().unwrap(), Some(second));
    store.delete_secret_key_check().unwrap();
    assert_eq!(store.secret_key_check().unwrap(), None);
}

#[test]
fn requests_count_by_requester_pending_and_recent() {
    let store = store();
    for (id, scope, created_at) in [
        ("r1", "global", 100),
        ("r2", "global", 200),
        ("r3", "project:p", 300),
    ] {
        store
            .insert_secret_request(&request(id, scope, "TOKEN", created_at))
            .unwrap();
    }
    let mut other = request("r4", "global", "TOKEN", 400);
    other.requester = Some("device:d1".to_owned());
    other.session = None;
    store.insert_secret_request(&other).unwrap();
    assert!(store.resolve_secret_request("r1", "declined", 500).unwrap());

    let (me, session) = ("owner", Some("session-1"));
    let pending = |scope, requester, session| {
        store
            .count_pending_secret_requests(scope, requester, session)
            .unwrap()
    };
    assert_eq!(pending("global", me, session), 1);
    assert_eq!(pending("project:p", me, session), 1);
    assert_eq!(pending("global", me, None), 0);
    assert_eq!(pending("global", "device:d1", None), 1);
    assert_eq!(store.count_secret_requests_since(me, 150).unwrap(), 2);
    assert_eq!(store.count_secret_requests_since(me, 0).unwrap(), 3);
    assert_eq!(
        store.count_secret_requests_since("device:d1", 0).unwrap(),
        1
    );
}
