//! Links and requests: a link shares one value, removal of a linked secret needs its waiver, and requests resolve once.

use massorca_core_store::Store;
use massorca_core_vault::{Scope, SecretValue, Vault, VaultError};

const VALUE: &str = "sk-test-value-0123456789";

fn vault(dir: &std::path::Path) -> (Vault, Store) {
    let store = Store::open_in_memory().expect("store");
    (Vault::new(store.clone(), dir.join("vault"), true), store)
}

fn value(text: &str) -> SecretValue {
    SecretValue::new(text.to_owned()).expect("value")
}

fn scope(text: &str) -> Scope {
    text.parse().expect("scope")
}

fn env(vault: &Vault, project: &str) -> Vec<(String, String)> {
    vault
        .environment_for(project)
        .expect("environment")
        .into_iter()
        .map(|(name, value)| (name, value.expose().to_owned()))
        .collect()
}

fn pair(name: &str, value: &str) -> (String, String) {
    (name.to_owned(), value.to_owned())
}

#[test]
fn links_share_one_value_and_removal_needs_the_waiver() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (vault, _) = vault(dir.path());
    vault
        .set(
            &scope("project:oppm"),
            "OPENAI_API_KEY",
            &value(VALUE),
            false,
        )
        .expect("set");
    let linked = vault
        .link(
            &scope("project:nexus"),
            "OPENAI_API_KEY",
            &scope("project:oppm"),
        )
        .expect("link");
    assert!(linked.entry.linked);
    assert_eq!(linked.entry.owner_scope, "project:oppm");
    vault
        .link(&scope("team:dev"), "OPENAI_API_KEY", &scope("project:oppm"))
        .expect("link");

    let through_link = vault
        .set(&scope("project:nexus"), "OPENAI_API_KEY", &value("x"), true)
        .unwrap_err();
    assert!(
        matches!(through_link, VaultError::Linked { ref owner, .. } if owner == "project:oppm"),
        "{through_link:?}"
    );
    assert_eq!(env(&vault, "nexus"), [pair("OPENAI_API_KEY", VALUE)]);

    let listed = vault.list(None).expect("list");
    let owner = listed
        .iter()
        .find(|entry| entry.scope == "project:oppm")
        .expect("owner");
    assert_eq!(owner.linked_scopes, ["project:nexus", "team:dev"]);

    let refused = vault
        .remove(&scope("project:oppm"), "OPENAI_API_KEY", false, false)
        .unwrap_err();
    match &refused {
        VaultError::HasLinks { linked, .. } => assert_eq!(linked, &["project:nexus", "team:dev"]),
        other => panic!("{other:?}"),
    }
    assert!(!refused.to_string().contains(VALUE));
    vault
        .unlink(&scope("team:dev"), "OPENAI_API_KEY")
        .expect("unlink");
    let removed = vault
        .remove(&scope("project:oppm"), "OPENAI_API_KEY", true, false)
        .expect("remove");
    assert_eq!(removed, ["project:nexus"]);
    assert_eq!(vault.list(None).expect("list"), Vec::new());
    assert_eq!(env(&vault, "nexus"), Vec::new());
}

#[test]
fn requests_resolve_when_the_name_is_stored_or_declined() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (vault, _) = vault(dir.path());
    let first = vault
        .request(
            &scope("project:p1"),
            "KEY",
            "Needed for the summary step.",
            Some("s1".to_owned()),
            "owner",
        )
        .expect("request");
    let other = vault
        .request(&scope("global"), "OTHER", "Why", None, "owner")
        .expect("request");
    assert!(matches!(
        vault
            .request(&scope("global"), "1BAD", "x", None, "owner")
            .unwrap_err(),
        VaultError::InvalidName
    ));
    assert!(matches!(
        vault
            .request(&scope("global"), "OK", "line\u{7}bell", None, "owner")
            .unwrap_err(),
        VaultError::InvalidReason
    ));

    let written = vault
        .set(&scope("project:p1"), "KEY", &value(VALUE), false)
        .expect("set");
    assert_eq!(written.resolved_requests, [first.id]);
    assert_eq!(vault.decline(&other.id).expect("decline").state, "declined");
    assert!(matches!(
        vault.decline(&other.id).unwrap_err(),
        VaultError::RequestResolved { .. }
    ));
    assert!(matches!(
        vault.decline("nope").unwrap_err(),
        VaultError::RequestNotFound(_)
    ));
    assert_eq!(vault.requests(false).expect("pending"), Vec::new());
    let states: Vec<String> = vault
        .requests(true)
        .expect("all")
        .into_iter()
        .map(|request| request.state)
        .collect();
    assert_eq!(states, ["stored", "declined"]);
}
