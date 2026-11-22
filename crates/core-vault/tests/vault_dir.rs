//! Windows: an existing vault folder and key from an earlier build get an owner-only DACL when opened, and a junction
//! at the vault path is refused, never followed.
#![cfg(windows)]

use massorca_core_store::Store;
use massorca_core_vault::{KeyError, KeyStore, LockReason, Vault};
use massorca_profile::has_owner_only_dacl;

#[test]
fn an_inherited_vault_folder_and_key_are_tightened_when_opened() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join("vault");
    std::fs::create_dir(&dir).expect("as an earlier build made it");
    std::fs::write(dir.join("key"), [9u8; 32]).expect("key");
    std::fs::write(dir.join("protection"), b"file\n").expect("protection");
    for path in [dir.clone(), dir.join("key")] {
        assert!(
            !has_owner_only_dacl(&path).expect("dacl"),
            "{} starts with inherited access",
            path.display()
        );
    }

    KeyStore::open_or_create(&dir, true).expect("the key opens");
    for path in [dir.clone(), dir.join("key"), dir.join("protection")] {
        assert!(
            has_owner_only_dacl(&path).expect("dacl"),
            "{} is owner-only",
            path.display()
        );
    }
}

#[test]
fn a_junction_at_the_vault_path_is_refused_and_never_followed() {
    let root = tempfile::tempdir().expect("tempdir");
    let target = root.path().join("elsewhere");
    std::fs::create_dir(&target).expect("target");
    std::fs::write(target.join("key"), [9u8; 32]).expect("key");
    std::fs::write(target.join("protection"), b"file\n").expect("protection");
    let junction = root.path().join("vault");
    let made = massorca_process::command("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&target)
        .output()
        .expect("mklink runs");
    assert!(made.status.success(), "{made:?}");

    let refused = KeyStore::open_or_create(&junction, true).unwrap_err();
    assert!(
        matches!(refused, KeyError::Locked(LockReason::UnsafeDirectory)),
        "{refused:?}"
    );
    assert!(matches!(
        KeyStore::reset(&junction).unwrap_err(),
        KeyError::Locked(LockReason::UnsafeDirectory)
    ));
    let vault = Vault::new(Store::open_in_memory().expect("store"), junction, true);
    assert!(vault.status().expect("status").locked);
    assert!(target.join("key").exists() && target.join("protection").exists());
    assert!(
        !has_owner_only_dacl(&target).expect("dacl"),
        "the junction's target was not touched"
    );
}
