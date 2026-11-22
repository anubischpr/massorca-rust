//! The vendor installer path of `massorca extension`: its own consent, what runs and what is recorded, a failing
//! installer, and uninstalling a CLI the package installed. The fake vendor program does the work, never a real one.

mod extension_support;
mod harness;

use extension_support::{FAKE_VENDOR, Host, KIT};
use predicates::str::contains;
use serde_json::json;

/// The install arguments that make the fake vendor copy itself to `acmecli` in the bin folder.
fn installing(host: &Host) -> Vec<String> {
    vec![
        "install".to_owned(),
        host.bin.path().to_string_lossy().into_owned(),
        "acmecli".to_owned(),
    ]
}

fn source_installing(host: &Host) -> String {
    let install = installing(host);
    host.write_dir(KIT, &install.iter().map(String::as_str).collect::<Vec<_>>())
}

fn install_log(host: &Host) -> String {
    std::fs::read_to_string(host.extensions_dir().join("data/acme.kit/install.log"))
        .expect("the installer's log")
}

#[test]
fn an_installer_needs_its_own_consent_and_nothing_runs_without_it() {
    let host = Host::new();
    let source = source_installing(&host);

    host.run(&["extension", "install", &source])
        .code(1)
        .stderr(contains("--yes --yes-run-installer"))
        .stderr(contains("installing runs this vendor command"));
    let error = host.json_error(&["extension", "install", &source, "--yes"]);

    assert_eq!(error["code"], "extension.installer_consent_required");
    assert!(
        error["message"]
            .as_str()
            .is_some_and(|message| message.contains("--yes-run-installer"))
    );
    assert_eq!(error["data"]["runs_installer"], true);
    assert!(!host.has_cli("acmecli"), "the installer must not have run");
    assert!(host.installed_ids().is_empty());
    assert!(!host.extensions_dir().join("pkgs/acme.kit").exists());
}

#[test]
fn with_both_flags_the_installer_runs_is_recorded_and_verified() {
    let host = Host::new();
    let source = source_installing(&host);
    assert!(!host.has_cli("acmecli"));

    let installed = host.json(&[
        "extension",
        "install",
        &source,
        "--yes",
        "--yes-run-installer",
    ]);

    assert!(
        host.has_cli("acmecli"),
        "the vendor installer put the CLI on PATH"
    );
    let record = &installed["cli"][0];
    assert_eq!(record["installed_by_package"], true);
    assert_eq!(record["detected_before_install"], false);
    assert_eq!(record["installer"]["exit_code"], 0);
    let command = json!([
        FAKE_VENDOR,
        "install",
        host.bin.path().to_string_lossy(),
        "acmecli"
    ]);
    assert_eq!(record["installer"]["command"], command);
    let shown = host.json(&["extension", "show", "acme.kit"]);
    assert_eq!(
        shown["installed"]["cli"][0]["installer"]["command"],
        command
    );
    assert!(shown["installed"]["cli"][0]["installer"]["ran_at_ms"].as_u64() > Some(0));
    assert_eq!(shown["agents"][0]["detected"], true);
    assert_eq!(shown["agents"][0]["cli_installed_by_package"], true);
    let log = install_log(&host);
    assert!(
        log.contains("installed acmecli into"),
        "the installer's output: {log:?}"
    );
    assert!(
        log.contains("fake-vendor 1.0"),
        "the verify output: {log:?}"
    );
    host.run(&["extension", "show", "acme.kit"])
        .success()
        .stdout(contains("installed by the package with"));
}

#[test]
fn a_failing_installer_leaves_nothing_installed() {
    let host = Host::new();
    let source = host.write_dir(KIT, &["fail", "7"]);

    let error = host.json_error(&[
        "extension",
        "install",
        &source,
        "--yes",
        "--yes-run-installer",
    ]);

    assert_eq!(error["code"], "extension.installer_failed");
    assert!(
        error["message"]
            .as_str()
            .is_some_and(|m| m.contains("exited with 7"))
    );
    assert!(host.installed_ids().is_empty());
    assert!(!host.extensions_dir().join("pkgs/acme.kit").exists());
    assert!(install_log(&host).contains("failing on purpose"));
    assert!(!host.has_cli("acmecli"));
}

#[test]
fn an_installer_that_leaves_no_cli_fails_the_verify_and_installs_nothing() {
    let host = Host::new();
    let source = host.write_dir(KIT, &["install-nothing"]);

    let error = host.json_error(&[
        "extension",
        "install",
        &source,
        "--yes",
        "--yes-run-installer",
    ]);

    assert_eq!(error["code"], "extension.verify_failed");
    assert!(host.installed_ids().is_empty());
    assert!(!host.extensions_dir().join("pkgs/acme.kit").exists());
}

#[test]
fn removing_offers_the_uninstall_of_a_cli_the_package_installed() {
    let host = Host::new();
    let source = source_installing(&host);
    host.json(&[
        "extension",
        "install",
        &source,
        "--yes",
        "--yes-run-installer",
    ]);

    let removed = host.json(&["extension", "remove", "acme.kit"]);

    assert_eq!(removed["cli"][0]["action"], "offered");
    assert_eq!(
        removed["cli"][0]["command"],
        json!([
            FAKE_VENDOR,
            "uninstall",
            host.bin.path().to_string_lossy(),
            "acmecli"
        ])
    );
    assert!(host.has_cli("acmecli"), "nothing was uninstalled");
    assert!(host.installed_ids().is_empty());
}

#[test]
fn uninstall_cli_runs_the_uninstaller_of_a_cli_the_package_installed() {
    let host = Host::new();
    let source = source_installing(&host);
    host.json(&[
        "extension",
        "install",
        &source,
        "--yes",
        "--yes-run-installer",
    ]);
    assert!(host.has_cli("acmecli"));

    let removed = host.json(&["extension", "remove", "acme.kit", "--uninstall-cli"]);

    assert_eq!(removed["cli"][0]["action"], "ran");
    assert_eq!(removed["cli"][0]["exit_code"], 0);
    assert_eq!(removed["failures"], json!([]));
    assert!(
        !host.has_cli("acmecli"),
        "the vendor uninstaller removed the CLI"
    );
    assert!(
        host.extensions_dir()
            .join("logs/acme.kit-uninstall.log")
            .exists()
    );
    assert!(host.installed_ids().is_empty());
}

#[test]
fn uninstall_cli_leaves_a_cli_the_package_did_not_install() {
    let host = Host::new();
    host.put_cli("acmecli");
    let source = source_installing(&host);
    host.json(&["extension", "install", &source, "--yes"]);

    let removed = host.json(&["extension", "remove", "acme.kit", "--uninstall-cli"]);

    assert_eq!(removed["cli"][0]["action"], "left_alone");
    assert!(
        host.has_cli("acmecli"),
        "a CLI that was already there stays"
    );
    assert!(
        !host
            .extensions_dir()
            .join("logs/acme.kit-uninstall.log")
            .exists()
    );
    host.run(&["extension", "list"])
        .success()
        .stdout(contains("no extensions installed"));
}
