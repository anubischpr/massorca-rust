//! A vault that lost its key stays locked, a row that will not open is named, and untrusted text has no invisible
//! characters.

use massorca_core_store::Store;
use massorca_core_vault::{LockReason, Scope, SecretValue, Vault, VaultError};

fn value(text: &str) -> SecretValue {
    SecretValue::new(text.to_owned()).expect("value")
}

fn scope(text: &str) -> Scope {
    text.parse().expect("scope")
}

fn is_missing_key(error: &VaultError) -> bool {
    matches!(error, VaultError::Locked(LockReason::MissingKey))
}

#[test]
fn a_vault_whose_key_directory_is_gone_stays_locked_and_is_never_rekeyed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open_in_memory().expect("store");
    let vault_dir = dir.path().join("vault");
    let vault = Vault::new(store.clone(), vault_dir.clone(), true);
    vault
        .set(&scope("global"), "TOKEN", &value("original"), false)
        .expect("set");
    std::fs::remove_dir_all(&vault_dir).expect("the key is lost");

    for vault in [vault, Vault::new(store.clone(), vault_dir.clone(), true)] {
        let status = vault.status().expect("status");
        assert!(status.locked, "{status:?}");
        assert_eq!(status.count, 1);
        let new = vault
            .set(&scope("global"), "NEW", &value("x"), false)
            .unwrap_err();
        assert!(is_missing_key(&new), "{new:?}");
        let rotate = vault
            .set(&scope("global"), "TOKEN", &value("x"), true)
            .unwrap_err();
        assert!(is_missing_key(&rotate), "{rotate:?}");
        let link = vault
            .link(&scope("project:p1"), "TOKEN", &scope("global"))
            .unwrap_err();
        assert!(is_missing_key(&link), "{link:?}");
        let env = vault.environment_for("p1").unwrap_err();
        assert!(is_missing_key(&env), "{env:?}");
        assert!(!vault_dir.exists(), "no key or protection file was created");
    }
    let rows = store.list_secrets().expect("rows");
    assert_eq!(rows.len(), 1, "the ciphertext is kept");
    assert!(store.list_secret_links().expect("links").is_empty());
}

#[test]
fn an_unparseable_protection_word_is_locked_not_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open_in_memory().expect("store");
    let vault_dir = dir.path().join("vault");
    Vault::new(store.clone(), vault_dir.clone(), true)
        .set(&scope("global"), "TOKEN", &value("original"), false)
        .expect("set");
    std::fs::write(vault_dir.join("protection"), b"bogus\n").expect("corrupt the word");

    let vault = Vault::new(store, vault_dir, true);
    let status = vault.status().expect("status");
    assert!(status.locked, "{status:?}");
    assert_eq!(status.protection, None);
    let error = vault
        .set(&scope("global"), "NEW", &value("x"), false)
        .unwrap_err();
    assert!(
        matches!(error, VaultError::Locked(LockReason::UnknownProtection)),
        "{error:?}"
    );
}

#[test]
fn a_row_that_will_not_open_is_unreadable_and_marks_nothing_used() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open_in_memory().expect("store");
    let vault = Vault::new(store.clone(), dir.path().join("vault"), true);
    for name in ["AAA", "ZZZ"] {
        vault
            .set(&scope("global"), name, &value("v"), false)
            .expect("set");
    }
    let meta = store
        .list_secrets()
        .expect("rows")
        .into_iter()
        .find(|meta| meta.name == "ZZZ")
        .expect("ZZZ");
    let row = store.secret_by_id(&meta.id).expect("row").expect("row");
    let mut ciphertext = row.ciphertext.clone();
    ciphertext[0] ^= 0xff;
    store
        .update_secret_value(&row.id, &row.nonce, &ciphertext, row.rotated_at)
        .expect("corrupt the row");

    let error = vault.environment_for("p1").unwrap_err();
    match &error {
        VaultError::Unreadable { scope, name } => {
            assert_eq!((scope.as_str(), name.as_str()), ("global", "ZZZ"));
        }
        other => panic!("expected Unreadable, got {other:?}"),
    }
    for meta in store.list_secrets().expect("rows") {
        assert_eq!(meta.used_at, None, "{} was not marked used", meta.name);
    }
}

#[test]
fn a_request_session_is_short_plain_text() {
    let dir = tempfile::tempdir().expect("tempdir");
    let vault = Vault::new(
        Store::open_in_memory().expect("store"),
        dir.path().join("vault"),
        true,
    );
    let ok = "s".repeat(128);
    vault
        .request(&scope("global"), "TOKEN", "needed", Some(ok))
        .expect("a 128-character session");
    for bad in [
        "s".repeat(129),
        "a\u{202E}b".to_owned(),
        "a\nb".to_owned(),
        "a\u{200B}".to_owned(),
    ] {
        let error = vault
            .request(&scope("global"), "TOKEN", "needed", Some(bad.clone()))
            .unwrap_err();
        assert!(matches!(error, VaultError::InvalidSession), "{bad:?}");
        assert!(!error.to_string().contains(&bad));
    }
}
