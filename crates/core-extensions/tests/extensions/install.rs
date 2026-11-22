//! Installing from each source, consent, and the vendor installer.

use massorca_core_extensions::codes;
use massorca_core_extensions::source::{Source, sha256_hex};
use massorca_ext_package::codes as format_codes;

use crate::fakes::FakeHost;
use crate::fixture::{agent_package, package_files, zip};
use crate::service::setup;

const WRONG: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[test]
fn a_detected_cli_registers_without_running_anything() {
    let s = setup(FakeHost::new().with_program("kit"));
    let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    s.install(&source, false).expect("installs");
    assert!(s.host.runs().is_empty());
    let lock = s.lock_json().expect("lockfile");
    let entry = &lock["packages"][0];
    assert_eq!(entry["id"], "acme.kit");
    assert_eq!(entry["enabled"], true);
    assert_eq!(entry["source"]["kind"], "dir");
    assert_eq!(entry["cli"][0]["detected_before_install"], true);
    assert_eq!(entry["cli"][0]["installed_by_package"], false);
    assert!(entry["cli"][0].get("installer").is_none());
    let path = entry["path"].as_str().expect("path");
    assert!(path.starts_with("pkgs/acme.kit/1.2.0-"));
    assert!(s.exists(&format!("{path}/agents/kit.json")));
}

#[test]
fn a_catalogue_entry_installs_from_the_binary() {
    let s = setup(FakeHost::new().with_program("codex"));
    s.install(&Source::Catalogue("massorca.codex".to_owned()), false)
        .expect("installs");
    let lock = s.lock_json().expect("lockfile");
    assert_eq!(lock["packages"][0]["source"]["ref"], "massorca.codex");
    let path = lock["packages"][0]["path"].as_str().expect("path");
    assert!(s.exists(&format!("{path}/agents/codex.json")));
}

#[test]
fn an_archive_needs_its_pinned_hash() {
    let s = setup(FakeHost::new().with_program("kit"));
    let bytes = zip(&package_files(&agent_package("acme", "kit", "kit")));
    let path = s.home.path().join("kit.mxt");
    std::fs::write(&path, &bytes).expect("archive");
    let source = Source::File(path);
    let consent = s.service.inspect(&source).expect("inspect").consent;
    let missing = s.service.install(&source, Some(&consent), false, None);
    assert_eq!(missing.expect_err("no pin").code(), codes::HASH_REQUIRED);
    let wrong = s
        .service
        .install(&source, Some(&consent), false, Some(WRONG));
    assert_eq!(
        wrong.expect_err("wrong pin").code(),
        format_codes::HASH_MISMATCH
    );
    let entry = s
        .service
        .install(&source, Some(&consent), false, Some(&sha256_hex(&bytes)))
        .expect("pinned install");
    assert_eq!(entry.sha256, sha256_hex(&bytes));
}

#[test]
fn a_folder_pin_must_match_its_canonical_hash() {
    let s = setup(FakeHost::new().with_program("kit"));
    let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    let consent = s.service.inspect(&source).expect("inspect").consent;
    let error = s
        .service
        .install(&source, Some(&consent), false, Some(WRONG));
    assert_eq!(
        error.expect_err("mismatch").code(),
        format_codes::HASH_MISMATCH
    );
    assert!(s.lock_json().is_none());
}

#[test]
fn consent_is_required_and_must_match_the_review() {
    let s = setup(FakeHost::new().with_program("kit"));
    let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    let missing = s.service.install(&source, None, true, None);
    assert_eq!(
        missing.expect_err("no consent").code(),
        codes::CONSENT_REQUIRED
    );
    let stale = s.service.install(&source, Some("stale"), true, None);
    assert_eq!(stale.expect_err("mismatch").code(), codes::CONSENT_MISMATCH);
    assert!(s.lock_json().is_none());
    assert!(!s.exists("pkgs/acme.kit"));
}

#[test]
fn running_an_installer_needs_its_own_consent() {
    let s = setup(FakeHost::new());
    let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    let error = s.install(&source, false).expect_err("installer consent");
    assert_eq!(error.code(), codes::INSTALLER_CONSENT_REQUIRED);
    assert!(s.host.runs().is_empty());
    assert!(s.lock_json().is_none());
}

#[test]
fn a_successful_installer_is_recorded() {
    let s = setup(FakeHost::new());
    let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    s.install(&source, true).expect("installs");
    assert_eq!(
        s.host.runs(),
        vec![
            vec!["npm", "install", "-g", "kit"],
            vec!["kit", "--version"]
        ]
    );
    let lock = s.lock_json().expect("lockfile");
    let cli = &lock["packages"][0]["cli"][0];
    assert_eq!(cli["detected_before_install"], false);
    assert_eq!(cli["installed_by_package"], true);
    assert_eq!(
        cli["installer"]["command"],
        serde_json::json!(["npm", "install", "-g", "kit"])
    );
    assert_eq!(cli["installer"]["exit_code"], 0);
    assert!(s.exists("data/acme.kit/install.log"));
}

#[test]
fn a_failed_installer_leaves_nothing_installed_but_its_log() {
    let s = setup(FakeHost::new().exit_for("install", 3));
    let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    let error = s.install(&source, true).expect_err("installer fails");
    assert_eq!(error.code(), codes::INSTALLER_FAILED);
    assert!(s.lock_json().is_none());
    assert!(!s.exists("pkgs/acme.kit"));
    let log = std::fs::read_to_string(s.service.layout().install_log("acme.kit")).expect("log");
    assert!(log.contains("npm install -g kit -> 3"));
}

#[test]
fn a_failed_verify_leaves_nothing_installed() {
    for host in [
        FakeHost::new().exit_for("--version", 1),
        FakeHost::new().without_binary(),
    ] {
        let s = setup(host);
        let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
        let error = s.install(&source, true).expect_err("verify fails");
        assert_eq!(error.code(), codes::VERIFY_FAILED);
        assert!(s.lock_json().is_none());
        assert!(!s.exists("pkgs/acme.kit"));
        let data = error.data().expect("the error says what ran");
        assert_eq!(data["installer_ran"], true);
        assert_eq!(data["cli_may_remain"], true);
        assert_eq!(
            data["command"],
            serde_json::json!(["npm", "install", "-g", "kit"])
        );
        let log = std::fs::read_to_string(s.service.layout().install_log("acme.kit"))
            .expect("the install log");
        assert!(log.contains("the CLI it installed may remain"), "{log}");
    }
}

#[test]
fn no_command_for_this_os_registers_without_running() {
    let mut s = setup(FakeHost::new());
    s.service = s.service.with_os(None);
    let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    s.install(&source, false).expect("installs");
    assert!(s.host.runs().is_empty());
    let lock = s.lock_json().expect("lockfile");
    assert_eq!(lock["packages"][0]["cli"][0]["installed_by_package"], false);
}

#[test]
fn the_same_package_or_agent_kind_cannot_be_installed_twice() {
    let s = setup(FakeHost::new().with_program("kit"));
    let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    s.install(&source, false).expect("first install");
    let again = s.install(&source, false).expect_err("already installed");
    assert_eq!(again.code(), codes::ALREADY_INSTALLED);
    let other = s.package_dir("beta", &agent_package("beta", "kit", "kit"));
    let conflict = s.install(&other, false).expect_err("kind conflict");
    assert_eq!(conflict.code(), codes::KIND_CONFLICT);
}

/// The agent package with its `verify` command removed, as a vendor that documents no check is declared.
fn without_verify(name: &str) -> serde_json::Value {
    let mut manifest = agent_package("acme", name, name);
    manifest["contributes"]["agentAdapters"][0]["cli"]
        .as_object_mut()
        .expect("the cli block")
        .remove("verify");
    manifest
}

#[test]
fn without_a_verify_command_an_installer_that_leaves_the_binary_registers() {
    let s = setup(FakeHost::new());
    let source = s.package_dir("kit", &without_verify("kit"));

    s.install(&source, true).expect("installs");

    let runs = s.host.runs();
    assert!(
        runs.iter()
            .all(|argv| argv.get(1).map(String::as_str) != Some("--version")),
        "no verify command runs: {runs:?}"
    );
    assert!(s.lock_json().is_some());
}

#[test]
fn without_a_verify_command_an_installer_that_leaves_no_binary_fails_and_installs_nothing() {
    let s = setup(FakeHost::new().without_binary());
    let source = s.package_dir("kit", &without_verify("kit"));

    let error = s
        .install(&source, true)
        .expect_err("the binary does not resolve");

    assert_eq!(error.code(), codes::VERIFY_FAILED);
    assert!(s.lock_json().is_none(), "nothing is registered");
}

#[test]
fn a_folder_changed_after_its_review_is_refused_for_the_reviewed_consent() {
    let s = setup(FakeHost::new().with_program("kit"));
    let source = s.package_dir("kit", &agent_package("acme", "kit", "kit"));
    let reviewed = s.service.inspect(&source).expect("the review").consent;
    let Source::Dir(dir) = &source else {
        unreachable!("a folder source");
    };
    std::fs::write(
        dir.join("media/logo.svg"),
        b"<svg>changed after review</svg>",
    )
    .expect("edit");

    let error = s
        .service
        .install(&source, Some(&reviewed), false, None)
        .expect_err("the consent is for what was reviewed");

    assert_eq!(error.code(), codes::CONSENT_MISMATCH);
    assert!(s.lock_json().is_none(), "nothing is registered");
}
