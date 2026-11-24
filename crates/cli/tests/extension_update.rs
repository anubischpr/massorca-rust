//! `massorca extension update`: the review and consent of a changed package, the up-to-date report and the refusals that
//! come before any change, through the CLI. No vendor command runs.

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

/// The consent of the review an update refused for lack of consent printed.
fn consent_of(refused: &Value) -> String {
    refused["data"]["consent"]
        .as_str()
        .expect("the review's consent")
        .to_owned()
}

#[test]
fn update_without_consent_prints_the_new_review_and_changes_nothing() {
    let host = Host::new();
    install_kit(&host);
    publish(&host, "1.3.0");

    let refused = host.json_error(&["extension", "update", KIT_ID]);

    assert_eq!(refused["code"], "extension.consent_required");
    assert_eq!(refused["data"]["version"], "1.3.0", "{refused}");
    assert_eq!(installed_version(&host), "1.2.0");
}

#[test]
fn update_with_yes_puts_the_new_version_in_place() {
    let host = Host::new();
    install_kit(&host);
    publish(&host, "1.3.0");

    let updated = host.json(&["extension", "update", KIT_ID, "--yes"]);

    assert_eq!(updated["id"], KIT_ID);
    assert_eq!(updated["version"], "1.3.0");
    assert_eq!(installed_version(&host), "1.3.0");
    assert_eq!(host.installed_ids(), [KIT_ID]);
}

#[test]
fn update_accepts_exactly_the_review_an_earlier_run_printed() {
    let host = Host::new();
    install_kit(&host);
    publish(&host, "1.3.0");
    let consent = consent_of(&host.json_error(&["extension", "update", KIT_ID]));

    let updated = host.json(&["extension", "update", KIT_ID, "--consent", &consent]);

    assert_eq!(updated["version"], "1.3.0");
    assert_eq!(installed_version(&host), "1.3.0");
}

#[test]
fn a_consent_from_before_a_further_change_is_refused() {
    let host = Host::new();
    install_kit(&host);
    publish(&host, "1.3.0");
    let stale = consent_of(&host.json_error(&["extension", "update", KIT_ID]));
    publish(&host, "1.4.0");

    let refused = host.json_error(&["extension", "update", KIT_ID, "--consent", &stale]);

    assert_eq!(refused["code"], "extension.consent_mismatch");
    assert_eq!(installed_version(&host), "1.2.0");
}

#[test]
fn an_unchanged_package_is_already_up_to_date_without_consent() {
    let host = Host::new();
    install_kit(&host);

    let current = host.json(&["extension", "update", KIT_ID]);

    assert_eq!(current["up_to_date"], true);
    assert_eq!(installed_version(&host), "1.2.0");
    host.run(&["extension", "update", KIT_ID])
        .success()
        .stdout(contains("acme.kit is already up to date"));
}

#[test]
fn updating_a_package_that_is_not_installed_is_refused() {
    let host = Host::new();

    let missing = host.json_error(&["extension", "update", KIT_ID, "--yes"]);

    assert_eq!(missing["code"], "extension.not_found");
}

#[test]
fn an_agent_run_cannot_update_and_nothing_changes() {
    let host = Host::new();
    install_kit(&host);
    publish(&host, "1.3.0");
    let agent = [
        ("MASSORCA_AGENT_SESSION", "session-1"),
        ("MASSORCA_AGENT_TOKEN", "not-a-real-token"),
    ];

    let refused = host.json_error_with_env(&["extension", "update", KIT_ID, "--yes"], &agent);

    assert_eq!(refused["code"], "extension.needs_person");
    assert_eq!(installed_version(&host), "1.2.0");
}
