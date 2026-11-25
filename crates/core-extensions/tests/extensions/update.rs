//! Updating an installed package in place: the new package's review is accepted again, the person's state is kept, and no
//! vendor command runs. Built-ins are refreshed from the embedded package and need no consent.

use std::fs;

use massorca_core_extensions::source::Source;
use massorca_core_extensions::{HookError, catalogue_agent_kinds, codes};
use serde_json::{Value, json};

use crate::builtin_refresh::outdated_gentle_shell;
use crate::fakes::FakeHost;
use crate::fixture::{agent_package, package_files, zip};
use crate::service::{Setup, setup};

fn kit_manifest(version: &str) -> Value {
    let mut manifest = agent_package("acme", "kit", "kit");
    manifest["version"] = json!(version);
    manifest
}

fn lock_bytes(s: &Setup) -> Vec<u8> {
    fs::read(s.service.layout().lock_path()).expect("the lockfile")
}

fn first_entry(s: &Setup) -> Value {
    s.lock_json().expect("the lockfile")["packages"][0].clone()
}

/// The number of package folders under `pkgs/acme.kit`, so a test sees a folder left behind by a refused update.
fn package_folders(s: &Setup) -> usize {
    fs::read_dir(s.service.layout().package_root("acme.kit")).map_or(0, Iterator::count)
}

/// The folder of one version of `acme.kit`, if it is on disk.
fn version_folder(s: &Setup, version: &str) -> Option<String> {
    fs::read_dir(s.service.layout().package_root("acme.kit"))
        .ok()?
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .find(|name| name.starts_with(&format!("{version}-")))
}

fn write_archive(path: &std::path::Path, manifest: &Value) {
    fs::write(path, zip(&package_files(manifest))).expect("the archive is written");
}

fn kit_refused_by_guard(_: &str) -> Result<(), HookError> {
    Err(HookError::new("agent.busy", "a kit session is live"))
}

#[test]
fn a_changed_folder_updates_in_place_and_keeps_its_state() {
    let s = setup(FakeHost::new());
    let source = s.package_dir("kit", &kit_manifest("1.2.0"));
    s.install(&source, false).expect("installs");
    s.service.set_enabled("acme.kit", false).expect("disables");
    let mut lock = s.lock_json().expect("the lockfile");
    lock["packages"][0]["x-note"] = json!("kept");
    fs::write(s.service.layout().lock_path(), lock.to_string()).expect("the lockfile");
    let before = first_entry(&s);
    let old_path = before["path"].as_str().expect("a path").to_owned();

    let source = s.package_dir("kit", &kit_manifest("1.3.0"));
    let consent = s.service.inspect(&source).expect("inspect").consent;
    let updated = s
        .service
        .update("acme.kit", Some(&consent), None)
        .expect("updates");

    assert_eq!(updated.version, "1.3.0");
    let entry = first_entry(&s);
    assert_eq!(entry["version"], "1.3.0");
    let path = entry["path"].as_str().expect("a path");
    assert!(path.starts_with("pkgs/acme.kit/1.3.0-"), "{path}");
    assert_ne!(path, old_path);
    assert_ne!(entry["sha256"], before["sha256"]);
    assert_eq!(entry["consent"], consent);
    assert_eq!(entry["enabled"], false);
    assert_eq!(entry["installed_at_ms"], before["installed_at_ms"]);
    assert_eq!(entry["cli"], before["cli"]);
    assert_eq!(entry["agent_kinds"], json!(["kit"]));
    assert_eq!(entry["x-note"], "kept");
    assert!(s.exists(&format!("{path}/agents/kit.json")));
    assert!(s.exists(&old_path), "the replaced version is kept");
    assert_eq!(entry["previous"]["version"], "1.2.0");
    assert_eq!(entry["previous"]["path"], old_path.as_str());
    assert_eq!(entry["previous"]["sha256"], before["sha256"]);
    assert_eq!(entry["previous"]["replaced_path"], entry["path"]);
    assert_eq!(entry["previous"]["consent"], before["consent"]);
    assert!(s.host.runs().is_empty(), "no vendor command runs");
}

#[test]
fn an_update_keeps_one_previous_version_and_drops_the_one_before() {
    let s = setup(FakeHost::new());
    let source = s.package_dir("kit", &kit_manifest("1.2.0"));
    s.install(&source, false).expect("installs");
    for version in ["1.3.0", "1.4.0"] {
        let source = s.package_dir("kit", &kit_manifest(version));
        let consent = s.service.inspect(&source).expect("inspect").consent;
        s.service
            .update("acme.kit", Some(&consent), None)
            .expect("updates");
    }

    let entry = first_entry(&s);
    assert_eq!(entry["version"], "1.4.0");
    assert_eq!(entry["previous"]["version"], "1.3.0");
    assert_eq!(
        package_folders(&s),
        2,
        "the current and the previous version"
    );
    assert!(
        version_folder(&s, "1.2.0").is_none(),
        "the version before the previous one is removed"
    );
}

#[test]
fn an_update_back_to_the_kept_version_reuses_its_folder() {
    let s = setup(FakeHost::new());
    let older = s.package_dir("kit", &kit_manifest("1.2.0"));
    s.install(&older, false).expect("installs");
    let newer = s.package_dir("kit", &kit_manifest("1.3.0"));
    let consent = s.service.inspect(&newer).expect("inspect").consent;
    s.service
        .update("acme.kit", Some(&consent), None)
        .expect("updates");

    let older = s.package_dir("kit", &kit_manifest("1.2.0"));
    let consent = s.service.inspect(&older).expect("inspect").consent;
    let back = s
        .service
        .update("acme.kit", Some(&consent), None)
        .expect("updates back");

    assert_eq!(back.version, "1.2.0");
    let entry = first_entry(&s);
    assert_eq!(entry["previous"]["version"], "1.3.0");
    assert_eq!(package_folders(&s), 2);
    assert!(s.exists(entry["path"].as_str().expect("a path")));
}

#[test]
fn the_same_package_again_is_up_to_date_and_the_lockfile_is_not_written() {
    let s = setup(FakeHost::new());
    let source = s.package_dir("kit", &kit_manifest("1.2.0"));
    s.install(&source, false).expect("installs");
    let consent = s.service.inspect(&source).expect("inspect").consent;
    let before = lock_bytes(&s);

    let error = s
        .service
        .update("acme.kit", Some(&consent), None)
        .expect_err("already up to date");

    assert_eq!(error.code(), codes::UP_TO_DATE);
    assert_eq!(lock_bytes(&s), before);
    assert_eq!(package_folders(&s), 1);
}

#[test]
fn a_missing_or_stale_consent_is_refused_and_nothing_changes() {
    let s = setup(FakeHost::new());
    let installed = s.package_dir("kit", &kit_manifest("1.2.0"));
    let stale = s.service.inspect(&installed).expect("inspect").consent;
    s.install(&installed, false).expect("installs");
    s.package_dir("kit", &kit_manifest("1.3.0"));
    let before = lock_bytes(&s);
    let folders = package_folders(&s);

    let missing = s
        .service
        .update("acme.kit", None, None)
        .expect_err("no consent");
    assert_eq!(missing.code(), codes::CONSENT_REQUIRED);
    let mismatch = s
        .service
        .update("acme.kit", Some(&stale), None)
        .expect_err("the review of the old version");
    assert_eq!(mismatch.code(), codes::CONSENT_MISMATCH);

    assert_eq!(lock_bytes(&s), before);
    assert_eq!(package_folders(&s), folders, "no folder is staged");
}

#[test]
fn a_source_that_now_holds_another_package_is_refused_and_nothing_changes() {
    let s = setup(FakeHost::new());
    let source = s.package_dir("kit", &kit_manifest("1.2.0"));
    s.install(&source, false).expect("installs");
    let mut manifest = kit_manifest("1.3.0");
    manifest["id"] = json!("acme.other");
    let other = s.package_dir("kit", &manifest);
    let consent = s.service.inspect(&other).expect("inspect").consent;
    let before = lock_bytes(&s);

    let error = s
        .service
        .update("acme.kit", Some(&consent), None)
        .expect_err("another id");

    assert_eq!(error.code(), codes::INVALID_SOURCE);
    assert_eq!(lock_bytes(&s), before);
    assert_eq!(package_folders(&s), 1);
    assert!(
        !s.exists("pkgs/acme.other"),
        "nothing is staged for the other id"
    );
}

#[test]
fn a_file_source_needs_its_sha256_to_update() {
    let s = setup(FakeHost::new());
    let path = s.home.path().join("kit.mxt");
    write_archive(&path, &kit_manifest("1.2.0"));
    let source = Source::File(path.clone());
    let review = s.service.inspect(&source).expect("inspect");
    s.service
        .install(&source, Some(&review.consent), false, Some(&review.sha256))
        .expect("installs");

    write_archive(&path, &kit_manifest("1.3.0"));
    let review = s.service.inspect(&source).expect("inspect");
    let missing = s
        .service
        .update("acme.kit", Some(&review.consent), None)
        .expect_err("no pinned hash");
    assert_eq!(missing.code(), codes::HASH_REQUIRED);

    let updated = s
        .service
        .update("acme.kit", Some(&review.consent), Some(&review.sha256))
        .expect("updates");
    assert_eq!(updated.version, "1.3.0");
    assert!(s.host.runs().is_empty(), "no vendor command runs");
}

#[test]
fn an_unknown_id_is_not_installed() {
    let s = setup(FakeHost::new());

    let error = s
        .service
        .update("acme.kit", None, None)
        .expect_err("not installed");

    assert_eq!(error.code(), codes::NOT_INSTALLED);
}

#[test]
fn a_current_built_in_is_up_to_date_and_nothing_is_written() {
    let s = setup(FakeHost::new().with_program("gentle-shell"));
    s.install(
        &Source::Catalogue("massorca.gentle-shell".to_owned()),
        false,
    )
    .expect("installs");
    let before = lock_bytes(&s);

    let current = s
        .service
        .update("massorca.gentle-shell", None, None)
        .expect_err("already current");

    assert_eq!(current.code(), codes::UP_TO_DATE);
    assert_eq!(lock_bytes(&s), before);
}

#[test]
fn an_outdated_built_in_is_refreshed_in_place_and_keeps_its_state() {
    let s = setup(FakeHost::new().with_program("gentle-shell"));
    let old = outdated_gentle_shell(&s, |entry| entry["enabled"] = json!(false));
    let updated = s
        .service
        .update("massorca.gentle-shell", None, None)
        .expect("refreshes");

    let built_in = catalogue_agent_kinds()
        .into_iter()
        .find(|kind| kind.kind_id == "gentle-shell")
        .expect("a built-in kind");
    assert_eq!(updated.version, built_in.package_version);
    assert!(!updated.enabled, "the disabled state is kept");
    assert!(old.exists(), "the replaced version is kept for rollback");
    assert!(s.host.runs().is_empty(), "no vendor command runs");
}

#[test]
fn dropping_an_agent_kind_asks_the_remove_guards_first() {
    let s = setup(FakeHost::new());
    let source = s.package_dir("kit", &kit_manifest("1.2.0"));
    s.install(&source, false).expect("installs");
    let mut manifest = kit_manifest("1.3.0");
    manifest["contributes"]["agentAdapters"] = json!([]);
    manifest["permissions"] = json!([manifest["permissions"][0].clone()]);
    manifest["listing"]["connections"] = json!([]);
    let source = s.package_dir("kit", &manifest);
    let consent = s.service.inspect(&source).expect("inspect").consent;
    let before = lock_bytes(&s);

    let guard = s
        .service
        .register_remove_guard("agents", kit_refused_by_guard);
    let refused = s
        .service
        .update("acme.kit", Some(&consent), None)
        .expect_err("a guard refuses");
    assert_eq!(refused.code(), "agent.busy");
    assert_eq!(lock_bytes(&s), before);

    drop(guard);
    s.service
        .update("acme.kit", Some(&consent), None)
        .expect("updates once no guard refuses");
    assert!(first_entry(&s).get("agent_kinds").is_none());
}
