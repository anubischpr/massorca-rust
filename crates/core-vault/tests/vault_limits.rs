//! Names that one process would receive together never differ only by case, and requests are limited per requester.

use massorca_core_store::{SecretRequestRow, Store, now_ms};
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

fn old_request(
    id: &str,
    scope: &str,
    session: &str,
    state: &str,
    created_at: i64,
) -> SecretRequestRow {
    SecretRequestRow {
        id: id.to_owned(),
        scope: scope.to_owned(),
        name: "N".to_owned(),
        reason: "why".to_owned(),
        session: Some(session.to_owned()),
        requester: Some("owner".to_owned()),
        state: state.to_owned(),
        created_at,
        resolved_at: (state != "pending").then_some(created_at),
    }
}

#[test]
fn pending_requests_are_capped_per_requester_and_scope_whatever_the_session() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open_in_memory().expect("store");
    let vault = Vault::new(store.clone(), dir.path().join("vault"), true);
    // Older than a minute, so only the pending cap applies; each from a different session.
    let two_minutes_ago = now_ms() - 120_000;
    for index in 0..MAX_PENDING_REQUESTS {
        store
            .insert_secret_request(&old_request(
                &format!("old-{index}"),
                "project:p",
                &format!("s{index}"),
                "pending",
                two_minutes_ago,
            ))
            .expect("an old request");
    }
    let refused = vault
        .request(
            &scope("project:p"),
            "MORE",
            "why",
            Some("fresh".to_owned()),
            "owner",
        )
        .unwrap_err();
    assert!(
        matches!(refused, VaultError::RequestLimit(_)),
        "{refused:?}"
    );
    let row = vault
        .request(&scope("global"), "MORE", "why", None, "owner")
        .expect("another scope has its own cap");
    assert_eq!(row.requester.as_deref(), Some("owner"));
    vault
        .request(&scope("project:p"), "MORE", "why", None, "agent:a1")
        .expect("an agent session has its own quota");
}

#[test]
fn answered_requests_after_7_days_and_pending_after_30_are_pruned() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open_in_memory().expect("store");
    let vault = Vault::new(store.clone(), dir.path().join("vault"), true);
    let day = 24 * 60 * 60 * 1000;
    let now = now_ms();
    for (id, state, age_days) in [
        ("answered-8d", "declined", 8),
        ("answered-6d", "stored", 6),
        ("pending-31d", "pending", 31),
        ("pending-29d", "pending", 29),
    ] {
        store
            .insert_secret_request(&old_request(id, "global", "s", state, now - age_days * day))
            .expect("an old request");
    }
    vault
        .request(&scope("global"), "NEW", "why", None, "owner")
        .expect("request");
    let mut left: Vec<String> = vault
        .requests(true)
        .expect("requests")
        .into_iter()
        .map(|row| row.id)
        .filter(|id| id.starts_with("answered") || id.starts_with("pending"))
        .collect();
    left.sort();
    assert_eq!(left, ["answered-6d", "pending-29d"]);
}

fn stored_row(id: &str, scope: &str, name: &str) -> massorca_core_store::SecretRow {
    massorca_core_store::SecretRow {
        id: id.to_owned(),
        scope: scope.to_owned(),
        name: name.to_owned(),
        nonce: vec![0; 24],
        ciphertext: vec![0; 32],
        created_at: 1,
        rotated_at: 1,
        used_at: None,
    }
}

#[test]
fn a_case_clash_stored_by_an_older_build_is_reported_and_refused_at_injection() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open_in_memory().expect("store");
    let vault = Vault::new(store.clone(), dir.path().join("vault"), true);
    store
        .insert_secret(&stored_row("a", "global", "TOKEN"))
        .expect("row");
    store
        .insert_secret(&stored_row("b", "project:p1", "token"))
        .expect("row");
    store
        .insert_secret(&stored_row("c", "project:p2", "Other"))
        .expect("row");
    store
        .insert_secret(&stored_row("d", "project:p3", "OTHER"))
        .expect("row");

    let clashes = vault.name_clashes().expect("clashes");
    assert_eq!(
        clashes.len(),
        1,
        "projects never share a process: {clashes:?}"
    );
    let error = vault.environment_for("p1").unwrap_err();
    match &error {
        VaultError::NameClash { clash } => {
            let named: Vec<(&str, &str)> = clash
                .iter()
                .map(|entry| (entry.scope.as_str(), entry.name.as_str()))
                .collect();
            assert_eq!(named, [("global", "TOKEN"), ("project:p1", "token")]);
        }
        other => panic!("expected NameClash, got {other:?}"),
    }
    assert!(error.to_string().contains("differ only by case"));
    for meta in store.list_secrets().expect("rows") {
        assert_eq!(meta.used_at, None, "{} was not handed out", meta.name);
    }
}

#[test]
fn a_dangerous_name_is_refused_when_set_linked_or_requested() {
    let dir = tempfile::tempdir().expect("tempdir");
    let vault = vault(dir.path());
    let is_reserved = |error: &VaultError| matches!(error, VaultError::ReservedName { .. });
    for name in [
        "PATH",
        "Node_Options",
        "LD_PRELOAD",
        "ANTHROPIC_BASE_URL",
        "GIT_ASKPASS",
    ] {
        let set = vault
            .set(&scope("global"), name, &value(), false)
            .unwrap_err();
        assert!(is_reserved(&set), "{name}: {set:?}");
        let requested = vault
            .request(&scope("global"), name, "why", None, "owner")
            .unwrap_err();
        assert!(is_reserved(&requested), "{name}: {requested:?}");
        let linked = vault
            .link(&scope("project:p"), name, &scope("global"))
            .unwrap_err();
        assert!(is_reserved(&linked), "{name}: {linked:?}");
    }
    vault
        .set(&scope("global"), "OPENAI_API_KEY", &value(), false)
        .expect("an API key is what the vault is for");
}
