//! The key check: a key planted in the vault directory is refused, a vault from before the check accepts its key only
//! if every secret opens with it, and an empty vault's key can be reset.

use massorca_core_store::{SecretKeyCheck, Store};
use massorca_core_vault::{LockReason, Scope, SecretValue, Vault, VaultError};

fn value(text: &str) -> SecretValue {
    SecretValue::new(text.to_owned()).expect("value")
}

fn scope(text: &str) -> Scope {
    text.parse().expect("scope")
}

fn is_mismatch(error: &VaultError) -> bool {
    matches!(error, VaultError::Locked(LockReason::KeyMismatch))
}

fn plant_key(vault_dir: &std::path::Path) {
    std::fs::remove_file(vault_dir.join("key")).expect("the old key goes");
    std::fs::write(vault_dir.join("key"), [7u8; 32]).expect("a key someone else knows");
    std::fs::write(vault_dir.join("protection"), b"file\n").expect("protection names it");
}

#[test]
fn a_planted_key_leaves_the_vault_locked_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open_in_memory().expect("store");
    let vault_dir = dir.path().join("vault");
    Vault::new(store.clone(), vault_dir.clone(), true)
        .set(&scope("global"), "TOKEN", &value("original"), false)
        .expect("set");
    let check = store.secret_key_check().expect("check").expect("recorded");
    let row = store
        .find_secret("global", "TOKEN")
        .expect("row")
        .expect("row");
    plant_key(&vault_dir);

    let vault = Vault::new(store.clone(), vault_dir.clone(), true);
    assert!(vault.status().expect("status").locked);
    let new = vault
        .set(&scope("global"), "NEW", &value("x"), false)
        .unwrap_err();
    assert!(is_mismatch(&new), "{new:?}");
    let rotate = vault
        .set(&scope("global"), "TOKEN", &value("x"), true)
        .unwrap_err();
    assert!(is_mismatch(&rotate), "{rotate:?}");
    assert!(is_mismatch(&vault.environment_for("p").unwrap_err()));

    assert_eq!(store.secret_key_check().expect("check"), Some(check));
    assert_eq!(store.list_secrets().expect("rows").len(), 1);
    assert_eq!(
        store.find_secret("global", "TOKEN").expect("row"),
        Some(row)
    );
    assert_eq!(
        std::fs::read(vault_dir.join("key")).expect("key"),
        [7u8; 32],
        "the planted key is left for the owner to inspect"
    );
}

#[test]
fn a_vault_without_a_check_accepts_its_key_once_when_every_secret_opens() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open_in_memory().expect("store");
    let vault_dir = dir.path().join("vault");
    Vault::new(store.clone(), vault_dir.clone(), true)
        .set(&scope("global"), "TOKEN", &value("original"), false)
        .expect("set");
    store
        .delete_secret_key_check()
        .expect("as before the check");

    let vault = Vault::new(store.clone(), vault_dir, true);
    assert!(!vault.status().expect("status").locked);
    assert!(store.secret_key_check().expect("check").is_some());
}

#[test]
fn a_vault_without_a_check_refuses_a_key_that_does_not_open_its_secrets() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open_in_memory().expect("store");
    let vault_dir = dir.path().join("vault");
    Vault::new(store.clone(), vault_dir.clone(), true)
        .set(&scope("global"), "TOKEN", &value("original"), false)
        .expect("set");
    store
        .delete_secret_key_check()
        .expect("as before the check");
    plant_key(&vault_dir);

    let vault = Vault::new(store.clone(), vault_dir, true);
    assert!(vault.status().expect("status").locked);
    assert!(is_mismatch(
        &vault
            .set(&scope("global"), "NEW", &value("x"), false)
            .unwrap_err()
    ));
    assert_eq!(store.secret_key_check().expect("check"), None);
}

#[test]
fn reset_key_is_refused_while_a_secret_exists_and_frees_an_empty_vault() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open_in_memory().expect("store");
    let vault_dir = dir.path().join("vault");
    let vault = Vault::new(store.clone(), vault_dir.clone(), true);
    vault
        .set(&scope("global"), "TOKEN", &value("original"), false)
        .expect("set");
    let refused = vault.reset_key().unwrap_err();
    assert!(
        matches!(refused, VaultError::VaultNotEmpty { count: 1 }),
        "{refused:?}"
    );
    assert!(vault_dir.join("key").exists());

    vault
        .remove(&scope("global"), "TOKEN", false, false)
        .expect("remove");
    plant_key(&vault_dir);
    assert!(
        vault.status().expect("status").locked || {
            // The running vault keeps its opened key; a restart sees the planted one.
            Vault::new(store.clone(), vault_dir.clone(), true)
                .status()
                .expect("status")
                .locked
        }
    );

    vault.reset_key().expect("an empty vault resets");
    for file in ["key", "protection", "id"] {
        assert!(!vault_dir.join(file).exists(), "{file} was removed");
    }
    assert_eq!(store.secret_key_check().expect("check"), None);
    vault
        .set(&scope("global"), "TOKEN", &value("fresh"), false)
        .expect("a fresh key is created");
    assert!(!vault.status().expect("status").locked);
    let check: Option<SecretKeyCheck> = store.secret_key_check().expect("check");
    assert!(check.is_some());
}
