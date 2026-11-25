//! `massorca extension rollback` and `massorca extension log`: the review and consent of the version a rollback restores,
//! the rollback itself, the run log, the refusal of an agent run, and the version each package can roll back to, through
//! the CLI. No vendor command runs.

mod extension_support;
mod harness;

use std::fs;

use extension_support::{Host, KIT};
use predicates::str::contains;
use serde_json::{Value, json};

const KIT_ID: &str = "acme.kit";

/// Installs the test package from its folder, with `--yes`, and returns its `dir:` source.
fn install_kit(host: &Host) -> String {
    let source = host.write_dir(KIT, &[]);
    host.json(&["extension", "install", &source, "--yes"]);
    source
}

/// Rewrites the package folder as release `version` of the same package, as a publisher's new version would.
fn publish(host: &Host, version: &str) {
    let mut manifest = KIT.manifest(host.bin.path(), &[]);
    manifest["version"] = json!(version);
    let dir = host.packages.path().join(KIT.id);
    fs::write(dir.join("extension.json"), manifest.to_string()).expect("the new manifest");
}

fn installed_version(host: &Host) -> String {
    host.json(&["extension", "show", KIT_ID])["installed"]["version"]
        .as_str()
        .expect("an installed version")
        .to_owned()
}

/// Installs 1.2.0 and updates it to 1.3.0, so 1.2.0 is the version a rollback restores.
fn updated(host: &Host) {
    install_kit(host);
    publish(host, "1.3.0");
    host.json(&["extension", "update", KIT_ID, "--yes"]);
}

#[test]
fn rollback_without_consent_prints_the_restored_review_and_changes_nothing() {
    let host = Host::new();
    updated(&host);

    let refused = host.json_error(&["extension", "rollback", KIT_ID]);

    assert_eq!(refused["code"], "extension.consent_required");
    assert_eq!(refused["data"]["version"], "1.2.0", "{refused}");
    assert!(refused["data"]["consent"].as_str().is_some(), "{refused}");
    assert_eq!(installed_version(&host), "1.3.0");
}

#[test]
fn rollback_with_yes_restores_the_previous_version() {
    let host = Host::new();
    updated(&host);

    host.json(&["extension", "rollback", KIT_ID, "--yes"]);

    assert_eq!(installed_version(&host), "1.2.0");
}

#[test]
fn rollback_with_the_printed_consent_accepts_exactly_that_review() {
    let host = Host::new();
    updated(&host);
    let refused = host.json_error(&["extension", "rollback", KIT_ID]);
    let consent = refused["data"]["consent"].as_str().expect("the consent");

    host.json(&["extension", "rollback", KIT_ID, "--consent", consent]);

    assert_eq!(installed_version(&host), "1.2.0");
}

#[test]
fn rollback_with_nothing_kept_says_so_and_changes_nothing() {
    let host = Host::new();
    install_kit(&host);

    let refused = host.json_error(&["extension", "rollback", KIT_ID, "--yes"]);

    assert_eq!(refused["code"], "extension.no_previous_version");
    assert_eq!(installed_version(&host), "1.2.0");
}

#[test]
fn an_agent_run_cannot_roll_back_and_nothing_changes() {
    let host = Host::new();
    updated(&host);
    let agent = [
        ("MASSORCA_AGENT_SESSION", "session-1"),
        ("MASSORCA_AGENT_TOKEN", "not-a-real-token"),
    ];

    let refused = host.json_error_with_env(&["extension", "rollback", KIT_ID, "--yes"], &agent);

    assert_eq!(refused["code"], "extension.needs_person");
    assert_eq!(installed_version(&host), "1.3.0");
}

#[test]
fn log_lists_each_run_oldest_first_as_json_and_as_text() {
    let host = Host::new();
    updated(&host);
    host.json(&["extension", "rollback", KIT_ID, "--yes"]);

    let log: Value = host.json(&["extension", "log", KIT_ID]);
    let actions: Vec<&str> = log["runs"]
        .as_array()
        .expect("the runs")
        .iter()
        .map(|run| run["action"].as_str().expect("an action"))
        .collect();
    assert_eq!(actions, ["install", "update", "rollback"], "{log}");
    assert!(
        log["runs"][0].get("output").is_none(),
        "no vendor command ran"
    );

    host.run(&["extension", "log", KIT_ID])
        .success()
        .stdout(contains("rollback"));
}

#[test]
fn log_of_a_package_that_never_ran_is_empty() {
    let host = Host::new();

    let log = host.json(&["extension", "log", KIT_ID]);

    assert!(
        log["runs"].as_array().expect("the runs").is_empty(),
        "{log}"
    );
}

/// The `show` item of the test package, as JSON.
fn shown(host: &Host) -> Value {
    host.json(&["extension", "show", KIT_ID])
}

/// The lockfile as JSON, read from the profile's extensions folder.
fn lock(host: &Host) -> Value {
    let text = fs::read_to_string(host.extensions_dir().join("lock.json")).expect("the lockfile");
    serde_json::from_str(&text).expect("the lockfile as JSON")
}

/// Rewrites the lockfile's kept record of the test package with `edit`.
fn edit_kept(host: &Host, edit: impl FnOnce(&mut Value)) {
    let mut lock = lock(host);
    let entry = lock["packages"]
        .as_array_mut()
        .expect("the packages")
        .iter_mut()
        .find(|entry| entry["id"] == KIT_ID)
        .expect("the test package");
    edit(&mut entry["previous"]);
    fs::write(host.extensions_dir().join("lock.json"), lock.to_string()).expect("the lockfile");
}

#[test]
fn after_an_update_the_item_names_the_version_it_can_roll_back_to() {
    let host = Host::new();
    updated(&host);

    assert_eq!(shown(&host)["previous_version"], "1.2.0");
}

#[test]
fn after_a_rollback_the_item_names_the_version_the_rollback_replaced() {
    let host = Host::new();
    updated(&host);

    host.json(&["extension", "rollback", KIT_ID, "--yes"]);

    assert_eq!(shown(&host)["previous_version"], "1.3.0");
}

#[test]
fn a_fresh_install_has_no_previous_version() {
    let host = Host::new();
    install_kit(&host);

    assert!(
        shown(&host).get("previous_version").is_none(),
        "{}",
        shown(&host)
    );
}

#[test]
fn a_kept_version_whose_replaced_path_is_stale_has_no_previous_version() {
    let host = Host::new();
    updated(&host);
    edit_kept(&host, |kept| {
        kept["replaced_path"] = json!("pkgs/acme.kit/elsewhere")
    });

    assert!(
        shown(&host).get("previous_version").is_none(),
        "{}",
        shown(&host)
    );
}

#[test]
fn a_kept_version_whose_folder_is_missing_has_no_previous_version() {
    let host = Host::new();
    updated(&host);
    let kept_path = lock(&host)["packages"][0]["previous"]["path"]
        .as_str()
        .expect("the kept path")
        .to_owned();
    fs::remove_dir_all(host.extensions_dir().join(&kept_path)).expect("the kept folder is removed");

    assert!(
        shown(&host).get("previous_version").is_none(),
        "{}",
        shown(&host)
    );
}
