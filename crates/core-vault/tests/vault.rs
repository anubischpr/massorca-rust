//! The vault over a real store and a file key: set, rotate, the environment an agent gets,
//! and a key that will not open. No listing or error ever carries a value.

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
fn status_creates_nothing_until_the_first_secret() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (vault, _) = vault(dir.path());
    let status = vault.status().expect("status");
    assert_eq!(
        (status.protection, status.locked, status.count),
        (None, false, 0)
    );
    assert!(!dir.path().join("vault").exists());
    assert_eq!(env(&vault, "p1"), Vec::new());

    vault
        .set(&scope("global"), "TOKEN", &value(VALUE), false)
        .expect("set");
    let status = vault.status().expect("status");
    assert_eq!(status.protection.map(|p| p.as_str()), Some("file"));
    assert_eq!((status.locked, status.count), (false, 1));
}

#[test]
fn set_rotate_and_the_project_environment() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (vault, store) = vault(dir.path());
    for (scope_text, name, text) in [
        ("global", "TOKEN", "global-token"),
        ("global", "SHARED", "global-shared"),
        ("project:p1", "SHARED", "p1-shared"),
    ] {
        vault
            .set(&scope(scope_text), name, &value(text), false)
            .expect("set");
    }

    let again = vault
        .set(&scope("global"), "TOKEN", &value("x"), false)
        .unwrap_err();
    assert!(matches!(again, VaultError::NameInUse { .. }), "{again:?}");
    let missing = vault
        .set(&scope("global"), "NOPE", &value("x"), true)
        .unwrap_err();
    assert!(
        matches!(missing, VaultError::NotFound { .. }),
        "{missing:?}"
    );
    vault
        .set(&scope("global"), "TOKEN", &value("rotated"), true)
        .expect("rotate");

    assert_eq!(
        env(&vault, "p1"),
        [pair("SHARED", "p1-shared"), pair("TOKEN", "rotated")]
    );
    assert_eq!(
        env(&vault, "p2"),
        [pair("SHARED", "global-shared"), pair("TOKEN", "rotated")]
    );
    for meta in store.list_secrets().unwrap() {
        assert!(meta.used_at.is_some(), "{} was marked used", meta.name);
        let row = store.secret_by_id(&meta.id).unwrap().unwrap();
        assert!(
            !row.ciphertext.windows(7).any(|window| window == b"rotated"),
            "the store holds ciphertext only"
        );
    }
}

#[test]
fn a_key_that_will_not_open_locks_the_vault_and_keeps_the_ciphertext() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open_in_memory().expect("store");
    let vault_dir = dir.path().join("vault");
    Vault::new(store.clone(), vault_dir.clone(), true)
        .set(&scope("global"), "TOKEN", &value(VALUE), false)
        .expect("set");
    std::fs::write(vault_dir.join("key"), b"short").expect("corrupt the key");

    let reopened = Vault::new(store.clone(), vault_dir, true);
    assert!(reopened.status().expect("status").locked);
    assert!(matches!(
        reopened.environment_for("p1").unwrap_err(),
        VaultError::Locked(_)
    ));
    assert!(matches!(
        reopened
            .set(&scope("global"), "NEW", &value("x"), false)
            .unwrap_err(),
        VaultError::Locked(_)
    ));
    assert!(matches!(
        reopened
            .remove(&scope("global"), "TOKEN", false, false)
            .unwrap_err(),
        VaultError::Locked(_)
    ));
    assert_eq!(
        store.list_secrets().unwrap().len(),
        1,
        "the ciphertext is kept"
    );
    reopened
        .remove(&scope("global"), "TOKEN", false, true)
        .expect("forced removal");
    assert_eq!(store.list_secrets().unwrap().len(), 0);
}
