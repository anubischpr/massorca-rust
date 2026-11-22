use std::fs;
use std::path::{Path, PathBuf};

use massorca_core_vault::{
    Aad, KeyError, KeyStore, MasterKey, Protection, Scope, SecretValue, open, seal,
};

/// Proves two keys are equal without exposing their bytes.
fn same_key(left: &MasterKey, right: &MasterKey) -> bool {
    let aad = Aad::new("probe", &Scope::Global, "PROBE").expect("aad");
    let value = SecretValue::new("probe".to_owned()).expect("value");
    let sealed = seal(left, &aad, &value).expect("seal");
    open(right, &aad, &sealed.nonce, &sealed.ciphertext).is_ok()
}

fn vault_dir(root: &Path) -> PathBuf {
    root.join("profile").join("vault")
}

fn protection_word(dir: &Path) -> String {
    fs::read_to_string(dir.join("protection"))
        .expect("protection file")
        .trim()
        .to_owned()
}

#[test]
fn file_key_is_created_once_and_reopened() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = vault_dir(root.path());
    let (protection, first) = KeyStore::open_or_create(&dir, true).expect("create");
    assert_eq!(protection, Protection::File);
    assert_eq!(protection_word(&dir), "file");
    assert_eq!(fs::read(dir.join("key")).expect("key").len(), 32);
    let (again, second) = KeyStore::open_or_create(&dir, true).expect("reopen");
    assert_eq!(again, Protection::File);
    assert!(same_key(&first, &second));
}

#[test]
fn corrupted_file_key_is_locked_and_left_untouched() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = vault_dir(root.path());
    KeyStore::open_or_create(&dir, true).expect("create");
    fs::write(dir.join("key"), b"short").expect("corrupt");
    let error = KeyStore::open_or_create(&dir, true).expect_err("locked");
    assert!(matches!(error, KeyError::Locked(_)), "{error:?}");
    assert_eq!(fs::read(dir.join("key")).expect("key"), b"short");
    assert_eq!(protection_word(&dir), "file");
}

#[test]
fn missing_key_with_a_protection_file_is_locked() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = vault_dir(root.path());
    KeyStore::open_or_create(&dir, true).expect("create");
    fs::remove_file(dir.join("key")).expect("remove");
    let error = KeyStore::open_or_create(&dir, true).expect_err("locked");
    assert!(matches!(error, KeyError::Locked(_)), "{error:?}");
    assert!(!dir.join("key").exists());
    assert_eq!(protection_word(&dir), "file");
}

#[test]
fn key_without_a_protection_file_is_locked_and_kept() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = vault_dir(root.path());
    fs::create_dir_all(&dir).expect("dir");
    fs::write(dir.join("key"), [7u8; 32]).expect("key");
    let error = KeyStore::open_or_create(&dir, true).expect_err("locked");
    assert!(matches!(error, KeyError::Locked(_)), "{error:?}");
    assert_eq!(fs::read(dir.join("key")).expect("key"), [7u8; 32]);
    assert!(!dir.join("protection").exists());
}

#[test]
fn unknown_protection_word_is_locked_and_never_switches() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = vault_dir(root.path());
    fs::create_dir_all(&dir).expect("dir");
    fs::write(dir.join("protection"), "keychain\n").expect("protection");
    let error = KeyStore::open_or_create(&dir, true).expect_err("locked");
    assert!(matches!(error, KeyError::Locked(_)), "{error:?}");
    assert_eq!(
        fs::read_to_string(dir.join("protection")).expect("protection"),
        "keychain\n"
    );
    assert!(!dir.join("key").exists());
}

#[test]
fn recorded_file_protection_wins_over_the_default() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = vault_dir(root.path());
    let (_, first) = KeyStore::open_or_create(&dir, true).expect("create");
    // Without force_file a new vault would pick the OS store; this one stays File.
    let (protection, second) = KeyStore::open_or_create(&dir, false).expect("reopen");
    assert_eq!(protection, Protection::File);
    assert!(same_key(&first, &second));
}

#[test]
fn protection_names_round_trip() {
    for protection in [
        Protection::Dpapi,
        Protection::SecretService,
        Protection::File,
    ] {
        assert_eq!(protection.as_str().parse::<Protection>(), Ok(protection));
        assert_eq!(protection.to_string(), protection.as_str());
    }
    assert!("Keychain".parse::<Protection>().is_err());
}

#[cfg(windows)]
#[test]
fn dpapi_key_is_created_and_reopened_and_forcing_file_never_switches() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = vault_dir(root.path());
    let (protection, first) = KeyStore::open_or_create(&dir, false).expect("create");
    assert_eq!(protection, Protection::Dpapi);
    assert_eq!(protection_word(&dir), "dpapi");
    // The wrapped blob is not the raw 32-byte key.
    assert_ne!(fs::read(dir.join("key")).expect("key").len(), 32);
    let (again, second) = KeyStore::open_or_create(&dir, true).expect("reopen");
    assert_eq!(again, Protection::Dpapi);
    assert!(same_key(&first, &second));
}

#[cfg(windows)]
#[test]
fn corrupted_dpapi_blob_is_locked_and_left_untouched() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = vault_dir(root.path());
    KeyStore::open_or_create(&dir, false).expect("create");
    let mut blob = fs::read(dir.join("key")).expect("key");
    let last = blob.len() - 1;
    blob[last] ^= 0xff;
    fs::write(dir.join("key"), &blob).expect("corrupt");
    let error = KeyStore::open_or_create(&dir, false).expect_err("locked");
    assert!(matches!(error, KeyError::Locked(_)), "{error:?}");
    assert_eq!(fs::read(dir.join("key")).expect("key"), blob);
    assert_eq!(protection_word(&dir), "dpapi");
}

#[cfg(not(target_os = "linux"))]
#[test]
fn secret_service_protection_off_linux_is_locked() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = vault_dir(root.path());
    fs::create_dir_all(&dir).expect("dir");
    fs::write(dir.join("protection"), "secret-service").expect("protection");
    let error = KeyStore::open_or_create(&dir, false).expect_err("locked");
    assert!(matches!(error, KeyError::Locked(_)), "{error:?}");
    assert!(!dir.join("key").exists());
}

#[cfg(unix)]
#[test]
fn file_key_permissions_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().expect("tempdir");
    let dir = vault_dir(root.path());
    KeyStore::open_or_create(&dir, true).expect("create");
    let mode = |path: &Path| fs::metadata(path).expect("metadata").permissions().mode() & 0o777;
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&dir.join("key")), 0o600);
}

#[cfg(unix)]
#[test]
fn symlinked_key_file_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = vault_dir(root.path());
    KeyStore::open_or_create(&dir, true).expect("create");
    let real = root.path().join("elsewhere");
    fs::rename(dir.join("key"), &real).expect("move");
    std::os::unix::fs::symlink(&real, dir.join("key")).expect("symlink");
    let error = KeyStore::open_or_create(&dir, true).expect_err("locked");
    assert!(matches!(error, KeyError::Locked(_)), "{error:?}");
}
