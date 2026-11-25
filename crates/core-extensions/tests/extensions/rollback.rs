//! Rolling a package back to the version its last update replaced: the kept files come back, the current ones are kept
//! in their place, the person's state stays, and no vendor command runs.

use std::fs;

use massorca_core_extensions::codes;
use serde_json::{Value, json};

use crate::fakes::FakeHost;
use crate::fixture::agent_package;
use crate::service::{Setup, setup};

fn kit_manifest(version: &str) -> Value {
    let mut manifest = agent_package("acme", "kit", "kit");
    manifest["version"] = json!(version);
    manifest
}

fn first_entry(s: &Setup) -> Value {
    s.lock_json().expect("the lockfile")["packages"][0].clone()
}

fn lock_bytes(s: &Setup) -> Vec<u8> {
    fs::read(s.service.layout().lock_path()).expect("the lockfile")
}

/// The consent of the review of `version`, as its package source holds it now.
fn consent_of(s: &Setup, version: &str) -> String {
    let source = s.package_dir("kit", &kit_manifest(version));
    s.service.inspect(&source).expect("inspect").consent
}

/// Updates the installed package to `version`, with the consent of its review.
fn update_to(s: &Setup, version: &str) {
    let consent = consent_of(s, version);
    s.service
        .update("acme.kit", Some(&consent), None)
        .expect("updates");
}

/// An installed `acme.kit` 1.2.0, updated to 1.3.0, so 1.2.0 is the version a rollback restores.
fn updated() -> Setup {
    let s = setup(FakeHost::new());
    let source = s.package_dir("kit", &kit_manifest("1.2.0"));
    s.install(&source, false).expect("installs");
    update_to(&s, "1.3.0");
    s
}

#[test]
fn a_rollback_restores_the_previous_version_and_keeps_the_current_one_as_previous() {
    let s = setup(FakeHost::new());
    let source = s.package_dir("kit", &kit_manifest("1.2.0"));
    s.install(&source, false).expect("installs");
    s.service.set_enabled("acme.kit", false).expect("disables");
    let before = first_entry(&s);
    update_to(&s, "1.3.0");
    let consent = consent_of(&s, "1.2.0");

    let rolled = s
        .service
        .rollback("acme.kit", Some(&consent))
        .expect("rolls back");

    assert_eq!(rolled.version, "1.2.0");
    let entry = first_entry(&s);
    assert_eq!(entry["version"], "1.2.0");
    assert_eq!(entry["path"], before["path"]);
    assert_eq!(entry["sha256"], before["sha256"]);
    assert_eq!(entry["consent"], consent);
    assert_eq!(entry["previous"]["version"], "1.3.0");
    assert_eq!(entry["previous"]["replaced_path"], entry["path"]);
    assert_eq!(entry["enabled"], false);
    assert_eq!(entry["installed_at_ms"], before["installed_at_ms"]);
    assert_eq!(entry["cli"], before["cli"]);
    assert!(s.host.runs().is_empty(), "no vendor command runs");
}

#[test]
fn a_second_rollback_goes_forward_to_the_version_it_undid() {
    let s = updated();
    let consent = consent_of(&s, "1.2.0");
    s.service
        .rollback("acme.kit", Some(&consent))
        .expect("rolls back");
    let consent = consent_of(&s, "1.3.0");

    let forward = s
        .service
        .rollback("acme.kit", Some(&consent))
        .expect("rolls forward");

    assert_eq!(forward.version, "1.3.0");
    assert_eq!(first_entry(&s)["previous"]["version"], "1.2.0");
}

#[test]
fn a_package_without_a_previous_version_has_nothing_to_roll_back() {
    let s = setup(FakeHost::new());
    let source = s.package_dir("kit", &kit_manifest("1.2.0"));
    s.install(&source, false).expect("installs");
    let consent = consent_of(&s, "1.2.0");
    let before = lock_bytes(&s);

    let error = s
        .service
        .rollback("acme.kit", Some(&consent))
        .expect_err("nothing is kept");

    assert_eq!(error.code(), codes::NO_PREVIOUS_VERSION);
    assert_eq!(lock_bytes(&s), before);
}

#[test]
fn a_rollback_needs_the_consent_of_the_restored_review() {
    let s = updated();
    let before = lock_bytes(&s);
    let current = consent_of(&s, "1.3.0");

    let missing = s
        .service
        .rollback("acme.kit", None)
        .expect_err("no consent");
    assert_eq!(missing.code(), codes::CONSENT_REQUIRED);
    let review = missing.data().expect("the review the rollback needs");
    assert_eq!(review["version"], "1.2.0", "{review}");
    assert_eq!(review["consent"], consent_of(&s, "1.2.0"));
    let mismatch = s
        .service
        .rollback("acme.kit", Some(&current))
        .expect_err("the review of the version it replaces");
    assert_eq!(mismatch.code(), codes::CONSENT_MISMATCH);

    assert_eq!(lock_bytes(&s), before);
}

#[test]
fn an_unknown_id_is_not_installed() {
    let s = setup(FakeHost::new());

    let error = s
        .service
        .rollback("acme.kit", None)
        .expect_err("not installed");

    assert_eq!(error.code(), codes::NOT_INSTALLED);
}

#[test]
fn a_tampered_previous_version_is_refused_and_nothing_changes() {
    let s = updated();
    let kept = first_entry(&s)["previous"]["path"]
        .as_str()
        .expect("a kept path")
        .to_owned();
    fs::write(
        s.service.layout().resolve(&kept).join("agents/kit.json"),
        "{\"id\":\"changed\"}",
    )
    .expect("the kept file is changed");
    let before = lock_bytes(&s);
    let consent = consent_of(&s, "1.2.0");

    let error = s
        .service
        .rollback("acme.kit", Some(&consent))
        .expect_err("the kept files differ from the review");

    assert_eq!(error.code(), codes::TAMPERED);
    assert_eq!(lock_bytes(&s), before);
}

#[test]
fn a_kept_version_bound_to_another_path_is_stale_and_refused_with_the_lock_unchanged() {
    let s = updated();
    let mut lock = s.lock_json().expect("the lockfile");
    lock["packages"][0]["previous"]["replaced_path"] = json!("pkgs/acme.kit/9.9.9-ffffffff");
    fs::write(s.service.layout().lock_path(), lock.to_string()).expect("the lockfile");
    let before = lock_bytes(&s);
    let consent = consent_of(&s, "1.2.0");

    let error = s
        .service
        .rollback("acme.kit", Some(&consent))
        .expect_err("the kept version was kept beside another version");

    assert_eq!(error.code(), codes::NO_PREVIOUS_VERSION);
    assert_eq!(lock_bytes(&s), before);
}

#[test]
fn a_kept_folder_that_is_missing_is_refused_cleanly_with_the_lock_unchanged() {
    let s = updated();
    let kept = first_entry(&s)["previous"]["path"]
        .as_str()
        .expect("a kept path")
        .to_owned();
    fs::remove_dir_all(s.service.layout().resolve(&kept)).expect("the kept folder is removed");
    let before = lock_bytes(&s);
    let consent = consent_of(&s, "1.2.0");

    let error = s
        .service
        .rollback("acme.kit", Some(&consent))
        .expect_err("there is no folder to restore");

    assert_eq!(error.code(), codes::NO_PREVIOUS_VERSION, "{error}");
    assert_eq!(lock_bytes(&s), before);
}

#[test]
fn a_rollback_restores_the_adopted_flag_and_the_kept_versions_own_unknown_fields() {
    let s = updated();
    let mut lock = s.lock_json().expect("the lockfile");
    lock["packages"][0]["x-note"] = json!("entry");
    lock["packages"][0]["x-shared"] = json!("entry");
    lock["packages"][0]["previous"]["adopted"] = json!(true);
    lock["packages"][0]["previous"]["x-shared"] = json!("kept");
    fs::write(s.service.layout().lock_path(), lock.to_string()).expect("the lockfile");
    let consent = consent_of(&s, "1.2.0");

    s.service
        .rollback("acme.kit", Some(&consent))
        .expect("rolls back");

    let restored = first_entry(&s);
    assert_eq!(restored["adopted"], true);
    assert_eq!(
        restored["x-note"], "entry",
        "the entry's own unknown field stays"
    );
    assert_eq!(
        restored["x-shared"], "kept",
        "the restored version's value wins"
    );
}
