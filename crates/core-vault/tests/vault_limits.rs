//! Names that one process would receive together never differ only by case, and requests are limited per requester.

use massorca_core_store::{SecretRequestRow, Store};
use massorca_core_vault::{
    MAX_PENDING_REQUESTS, MAX_REQUESTS_PER_MINUTE, Scope, SecretValue, Vault, VaultError,
};

fn vault(dir: &std::path::Path) -> Vault {
    Vault::new(
        Store::open_in_memory().expect("store"),
        dir.join("vault"),
        true,
    )
}

fn value() -> SecretValue {
    SecretValue::new("v".to_owned()).expect("value")
}

fn scope(text: &str) -> Scope {
    text.parse().expect("scope")
}

fn is_clash(error: &VaultError) -> bool {
    matches!(error, VaultError::NameCaseClash { .. })
}

#[test]
fn a_name_differing_only_by_case_is_refused_where_one_process_gets_both() {
    let dir = tempfile::tempdir().expect("tempdir");
    let vault = vault(dir.path());
    vault
        .set(&scope("global"), "TOKEN", &value(), false)
        .expect("set");

    let same_scope = vault
        .set(&scope("global"), "token", &value(), false)
        .unwrap_err();
    assert!(is_clash(&same_scope), "{same_scope:?}");
    assert!(same_scope.to_string().contains("only by case"));
    let project = vault
        .set(&scope("project:p1"), "Token", &value(), false)
        .unwrap_err();
    assert!(is_clash(&project), "{project:?}");

    vault
        .set(&scope("project:p2"), "Api_Key", &value(), false)
        .expect("set");
    let global = vault
        .set(&scope("global"), "API_KEY", &value(), false)
        .unwrap_err();
    assert!(
        is_clash(&global),
        "global reaches every project: {global:?}"
    );

    vault
        .set(&scope("project:p3"), "OTHER", &value(), false)
        .expect("set");
    let link = vault
        .link(&scope("project:p2"), "OTHER", &scope("project:p3"))
        .map(|_| ())
        .and_then(|()| {
            vault
                .set(&scope("project:p2"), "other", &value(), false)
                .map(|_| ())
        })
        .unwrap_err();
    assert!(is_clash(&link), "a linked name counts: {link:?}");

    // Projects never share a process, and teams are not injected with a project.
    vault
        .set(&scope("project:p1"), "Api_Key", &value(), false)
        .expect("another project");
    vault
        .set(&scope("team:t"), "token", &value(), false)
        .expect("a team");
    // The same name in a narrower scope is the intended override.
    vault
        .set(&scope("project:p1"), "TOKEN", &value(), false)
        .expect("override");
}

#[test]
fn requests_are_limited_per_requester() {
    let dir = tempfile::tempdir().expect("tempdir");
    let vault = vault(dir.path());
    assert_eq!((MAX_PENDING_REQUESTS, MAX_REQUESTS_PER_MINUTE), (20, 10));
    for index in 0..MAX_REQUESTS_PER_MINUTE {
        vault
            .request(
                &scope("global"),
                &format!("N{index}"),
                "why",
                None,
                "device:a",
            )
            .expect("within the limit");
    }
    let refused = vault
        .request(&scope("global"), "MORE", "why", None, "device:a")
        .unwrap_err();
    assert!(
        matches!(refused, VaultError::RequestLimit(_)),
        "{refused:?}"
    );
    vault
        .request(&scope("global"), "MORE", "why", None, "device:b")
        .expect("another requester has its own limit");
}

#[test]
fn pending_requests_are_capped_per_scope_and_session() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open_in_memory().expect("store");
    let vault = Vault::new(store.clone(), dir.path().join("vault"), true);
    // Older than a minute, so only the pending cap applies.
    for index in 0..MAX_PENDING_REQUESTS {
        store
            .insert_secret_request(&SecretRequestRow {
                id: format!("old-{index}"),
                scope: "project:p".to_owned(),
                name: format!("N{index}"),
                reason: "why".to_owned(),
                session: Some("s1".to_owned()),
                requester: Some("owner".to_owned()),
                state: "pending".to_owned(),
                created_at: 1_000,
                resolved_at: None,
            })
            .expect("an old request");
    }
    let session = Some("s1".to_owned());
    let refused = vault
        .request(&scope("project:p"), "MORE", "why", session.clone(), "owner")
        .unwrap_err();
    assert!(
        matches!(refused, VaultError::RequestLimit(_)),
        "{refused:?}"
    );
    vault
        .request(&scope("global"), "MORE", "why", session, "owner")
        .expect("another scope has its own cap");
    let row = vault
        .request(
            &scope("project:p"),
            "MORE",
            "why",
            Some("s2".to_owned()),
            "owner",
        )
        .expect("another session has its own cap");
    assert_eq!(row.requester.as_deref(), Some("owner"));
}
