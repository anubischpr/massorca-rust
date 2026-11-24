//! `massorca extension`: the catalogue, the review, the consent rules and the install, list, show, enable, disable and
//! remove flows, each through the CLI. The installer flows are in `extension_installer.rs`.

mod extension_support;
mod harness;

use std::fs;

use extension_support::{Host, KIT, Package};
use harness::stdout_text;
use predicates::str::contains;
use serde_json::{Value, json};

/// The install arguments that would leave a marker file if the installer ran.
fn marking_install(host: &Host) -> Vec<String> {
    vec![
        "record".to_owned(),
        host.marker().to_string_lossy().into_owned(),
        "ran".to_owned(),
    ]
}

fn args_of(owned: &[String]) -> Vec<&str> {
    owned.iter().map(String::as_str).collect()
}

fn ids_of(list: &Value) -> Vec<&str> {
    list["extensions"]
        .as_array()
        .expect("an extensions array")
        .iter()
        .map(|item| item["id"].as_str().expect("an id"))
        .collect()
}

#[test]
fn the_catalogue_lists_the_four_agent_packages_and_ext_is_an_alias() {
    let host = Host::new();

    let catalogue = host.json(&["extension", "catalogue"]);
    let aliased = host.json(&["ext", "catalogue"]);

    assert_eq!(
        ids_of(&catalogue),
        [
            "massorca.claude",
            "massorca.codex",
            "massorca.antigravity",
            "massorca.gentle-shell"
        ]
    );
    assert_eq!(catalogue, aliased);
    assert!(
        catalogue["extensions"]
            .as_array()
            .expect("items")
            .iter()
            .all(|item| item["state"] == "available")
    );
    host.run(&["ext", "catalogue"])
        .success()
        .stdout(contains("massorca.claude"))
        .stdout(contains("available"));
    assert!(host.installed_ids().is_empty());
}

#[test]
fn show_describes_a_catalogue_entry_and_names_a_missing_one() {
    let host = Host::new();

    let item = host.json(&["extension", "show", "massorca.codex"]);
    let missing = host.json_error(&["extension", "show", "acme.nothing"]);

    assert_eq!(item["state"], "available");
    assert_eq!(item["agents"][0]["id"], "codex");
    assert_eq!(item["agents"][0]["detected"], false);
    assert_eq!(missing["code"], "extension.not_found");
    host.run(&["extension", "show", "massorca.codex"])
        .success()
        .stdout(contains("agent codex: not detected"));
}

#[test]
fn inspect_prints_the_review_and_changes_nothing() {
    let host = Host::new();
    let source = host.write_dir(KIT, &["install"]);

    let review = host.json(&["extension", "inspect", &source]);

    assert_eq!(review["id"], "acme.kit");
    assert_eq!(review["risk"]["level"], "high");
    assert_eq!(review["runs_installer"], false);
    assert_eq!(review["cli"][0]["state"], "unsupported");
    assert!(!host.has_cli("acmecli"));
}

#[test]
fn inspect_of_a_built_in_shows_its_install_verify_and_uninstall_commands() {
    let host = Host::new();
    let text = stdout_text(
        &host
            .run(&["extension", "inspect", "catalogue:massorca.claude"])
            .success(),
    );
    assert!(text.contains("verify: "), "{text}");
    assert!(text.contains("uninstall on this system: "), "{text}");
    assert!(!host.has_cli("claude"));
}

#[test]
fn inspect_of_codex_and_gentle_shell_shows_their_npm_uninstall() {
    let host = Host::new();
    for (source, package) in [
        ("catalogue:massorca.codex", "@openai/codex"),
        ("catalogue:massorca.gentle-shell", "gentle-pi"),
    ] {
        let text = stdout_text(&host.run(&["extension", "inspect", source]).success());
        let line = text
            .lines()
            .find(|line| line.contains("uninstall on this system: "))
            .unwrap_or_else(|| panic!("{source} shows an uninstall: {text}"));
        assert!(
            line.contains("npm uninstall -g") && line.contains(package),
            "{line}"
        );
    }
    assert!(!host.has_cli("codex") && !host.has_cli("gentle-shell"));
}

#[test]
fn inspect_reads_a_relative_dir_source_from_the_callers_folder() {
    let host = Host::new();
    host.write_dir(KIT, &["fail"]);
    let mut command = host.sandbox.command("default");
    command
        .env("PATH", host.bin.path())
        .current_dir(host.packages.path())
        .args(["--json", "extension", "inspect", "dir:acme.kit"]);

    let outcome = harness::execute(&mut command).success();

    assert_eq!(
        harness::single_document(&outcome)["result"]["id"],
        "acme.kit"
    );
}

#[test]
fn install_without_yes_prints_the_review_and_installs_nothing() {
    let host = Host::new();
    host.put_cli("acmecli");
    let install = marking_install(&host);
    let source = host.write_dir(KIT, &args_of(&install));

    host.run(&["extension", "install", &source])
        .code(1)
        .stderr(contains("acme.kit"))
        .stderr(contains("risk: high"))
        .stderr(contains("extension.consent_required"))
        .stderr(contains("--yes"));
    let error = host.json_error(&["extension", "install", &source]);

    assert_eq!(error["code"], "extension.consent_required");
    assert_eq!(
        error["data"]["id"], "acme.kit",
        "the review travels as data"
    );
    assert!(error["data"]["consent"].is_string());
    assert!(host.installed_ids().is_empty());
    assert!(!host.extensions_dir().join("pkgs").join("acme.kit").exists());
}

#[test]
fn a_detected_cli_installs_with_yes_and_runs_nothing() {
    let host = Host::new();
    host.put_cli("acmecli");
    let install = marking_install(&host);
    let source = host.write_dir(KIT, &args_of(&install));

    let installed = host.json(&["extension", "install", &source, "--yes"]);

    assert_eq!(installed["id"], "acme.kit");
    assert_eq!(installed["enabled"], true);
    assert_eq!(installed["cli"][0]["detected_before_install"], true);
    assert_eq!(installed["cli"][0]["installed_by_package"], false);
    assert!(!host.marker().exists(), "the installer must not run");
    assert_eq!(host.installed_ids(), ["acme.kit"]);
    let shown = host.json(&["extension", "show", "acme.kit"]);
    assert_eq!(shown["state"], "enabled");
    assert_eq!(shown["installed"]["source"]["kind"], "dir");
    assert!(shown["installed"]["cli"][0]["installer"].is_null());
    assert_eq!(shown["agents"][0]["detected"], true);
    host.run(&["extension", "list"])
        .success()
        .stdout(contains("acme.kit"))
        .stdout(contains("enabled"));
    host.run(&["extension", "show", "acme.kit"])
        .success()
        .stdout(contains("found on this host, nothing ran"));
}

#[test]
fn disable_and_enable_keep_the_package_and_say_when_nothing_changed() {
    let host = Host::new();
    host.put_cli("acmecli");
    let source = host.write_dir(KIT, &["fail"]);
    host.json(&["extension", "install", &source, "--yes"]);

    let off = host.json(&["extension", "disable", "acme.kit"]);
    let again = host.json(&["extension", "disable", "acme.kit"]);
    let listed = host.json(&["extension", "list"]);

    assert_eq!(off["changed"], true);
    assert_eq!(off["enabled"], false);
    assert_eq!(again["changed"], false);
    assert_eq!(listed["extensions"][0]["state"], "disabled");
    assert!(host.extensions_dir().join("pkgs").join("acme.kit").exists());
    host.run(&["extension", "disable", "acme.kit"])
        .success()
        .stdout(contains("was already disabled"));
    host.run(&["extension", "enable", "acme.kit"])
        .success()
        .stdout(contains("is now enabled"));
    assert_eq!(
        host.json(&["extension", "show", "acme.kit"])["state"],
        "enabled"
    );
    let missing = host.json_error(&["extension", "enable", "acme.nothing"]);
    assert_eq!(missing["code"], "extension.not_installed");
}

#[test]
fn remove_deletes_the_package_and_its_data() {
    let host = Host::new();
    host.put_cli("acmecli");
    let source = host.write_dir(KIT, &["fail"]);
    host.json(&["extension", "install", &source, "--yes"]);
    assert!(host.extensions_dir().join("pkgs").join("acme.kit").exists());

    let removed = host.json(&["extension", "remove", "acme.kit"]);

    assert_eq!(removed["id"], "acme.kit");
    assert_eq!(removed["failures"], json!([]));
    assert_eq!(removed["cli"][0]["action"], "left_alone");
    assert!(host.installed_ids().is_empty());
    assert!(!host.extensions_dir().join("pkgs").join("acme.kit").exists());
    assert!(
        host.has_cli("acmecli"),
        "a CLI the package did not install stays"
    );
    let again = host.json_error(&["extension", "remove", "acme.kit"]);
    assert_eq!(again["code"], "extension.not_installed");
    host.json(&["extension", "install", &source, "--yes"]);
    host.run(&["extension", "remove", "acme.kit"])
        .success()
        .stdout(contains("removed acme.kit 1.2.0"));
}

const OTHER: Package = Package::new("acme.other", "otheragent", "otherbin");

#[test]
fn a_tampered_package_is_listed_with_its_problem_and_never_fails_the_list() {
    let host = Host::new();
    host.put_cli("acmecli");
    host.put_cli("otherbin");
    let kit = host.write_dir(KIT, &["fail"]);
    host.json(&["extension", "install", &kit, "--yes"]);
    let other = host.write_dir(OTHER, &["fail"]);
    host.json(&["extension", "install", &other, "--yes"]);
    let lock: Value = serde_json::from_slice(
        &fs::read(host.extensions_dir().join("lock.json")).expect("the lockfile"),
    )
    .expect("json");
    let rel = lock["packages"][0]["path"].as_str().expect("a path");
    let manifest = host.extensions_dir().join(rel).join("extension.json");
    let mut bytes = fs::read(&manifest).expect("the installed manifest");
    bytes.push(b'\n');
    fs::write(&manifest, bytes).expect("the file is changed after install");

    let list = host.json(&["extension", "list"]);

    assert_eq!(ids_of(&list), ["acme.kit", "acme.other"]);
    assert_eq!(
        list["extensions"][0]["problem"]["code"],
        "extension.tampered"
    );
    assert!(list["extensions"][1]["problem"].is_null());
    host.run(&["extension", "list"])
        .success()
        .stdout(contains("acme.other"))
        .stdout(contains("differ from what was reviewed"));
    let shown = host.json(&["extension", "show", "acme.kit"]);
    assert_eq!(shown["problem"]["code"], "extension.tampered");
    host.run(&["extension", "show", "acme.kit"])
        .success()
        .stdout(contains("problem: the installed files of acme.kit differ"));
    host.json(&["extension", "remove", "acme.kit"]);
    assert_eq!(host.installed_ids(), ["acme.other"]);
}

#[test]
fn installing_twice_is_already_installed() {
    let host = Host::new();
    host.put_cli("acmecli");
    let source = host.write_dir(KIT, &["fail"]);
    host.json(&["extension", "install", &source, "--yes"]);

    let again = host.json_error(&["extension", "install", &source, "--yes"]);

    assert_eq!(again["code"], "extension.already_installed");
    assert_eq!(host.installed_ids(), ["acme.kit"]);
}

#[test]
fn a_second_package_with_the_same_agent_is_a_kind_conflict() {
    let host = Host::new();
    host.put_cli("acmecli");
    let first = host.write_dir(KIT, &["fail"]);
    let rival = host.write_dir(Package::new("acme.rival", "kitagent", "acmecli"), &["fail"]);
    host.json(&["extension", "install", &first, "--yes"]);

    let error = host.json_error(&["extension", "install", &rival, "--yes"]);

    assert_eq!(error["code"], "extension.kind_conflict");
    assert!(
        error["message"]
            .as_str()
            .is_some_and(|m| m.contains("acme.kit"))
    );
    assert_eq!(host.installed_ids(), ["acme.kit"]);
}

#[test]
fn an_archive_needs_its_hash_and_a_wrong_hash_is_refused() {
    let host = Host::new();
    host.put_cli("acmecli");
    let source = host.write_archive(KIT, &["fail"]);
    let hash = host.json(&["extension", "inspect", &source])["sha256"]
        .as_str()
        .expect("the archive's hash")
        .to_owned();
    let wrong = "0".repeat(64);

    let unpinned = host.json_error(&["extension", "install", &source, "--yes"]);
    let mismatch = host.json_error(&["extension", "install", &source, "--yes", "--sha256", &wrong]);

    assert_eq!(unpinned["code"], "extension.hash_required");
    assert_eq!(mismatch["code"], "extension.hash_mismatch");
    assert!(host.installed_ids().is_empty());
    let installed = host.json(&["extension", "install", &source, "--yes", "--sha256", &hash]);
    assert_eq!(installed["source"]["kind"], "file");
    assert_eq!(host.installed_ids(), ["acme.kit"]);
}

#[test]
fn bad_sources_and_usage_errors_are_reported_without_installing() {
    let host = Host::new();

    let nowhere = host.json_error(&["extension", "inspect", "somewhere"]);
    let missing = host.json_error(&["extension", "inspect", "catalogue:acme.nothing"]);
    let alone = host.run(&[
        "extension",
        "install",
        "catalogue:massorca.codex",
        "--yes-run-installer",
    ]);

    assert_eq!(nowhere["code"], "extension.invalid_source");
    assert_eq!(missing["code"], "extension.not_found");
    alone.code(2).stderr(contains("--yes"));
    assert!(stdout_text(&host.run(&["extension", "list"]).success()).contains("no extensions"));
}

#[test]
fn a_fingerprint_from_one_run_accepts_exactly_that_review_in_the_next() {
    let host = Host::new();
    host.put_cli("acmecli");
    let install = marking_install(&host);
    let source = host.write_dir(KIT, &args_of(&install));
    let review = host.json_error(&["extension", "install", &source]);
    let consent = review["data"]["consent"]
        .as_str()
        .expect("a fingerprint")
        .to_owned();

    host.run(&["extension", "install", &source])
        .code(1)
        .stderr(contains(format!("consent: {consent}")))
        .stderr(contains(format!("--consent {consent}")));
    let stale = host.json_error(&[
        "extension",
        "install",
        &source,
        "--consent",
        &"0".repeat(64),
    ]);
    let installed = host.json(&["extension", "install", &source, "--consent", &consent]);

    assert_eq!(stale["code"], "extension.consent_mismatch");
    assert_eq!(installed["id"], "acme.kit");
    let both = host.run(&[
        "extension",
        "install",
        &source,
        "--yes",
        "--consent",
        &consent,
    ]);
    both.code(2);
}

#[test]
fn the_review_quotes_every_argument_unambiguously() {
    let host = Host::new();
    let source = host.write_dir(KIT, &["install"]);
    let text = stdout_text(&host.run(&["extension", "inspect", &source]).success());
    assert!(text.contains("process [acmecli]"), "{text}");
    let built_in = stdout_text(
        &host
            .run(&["extension", "inspect", "catalogue:massorca.antigravity"])
            .success(),
    );
    let quoted = if cfg!(windows) {
        r#""irm https://antigravity.google/cli/install.ps1 | iex""#
    } else {
        r#""set -o pipefail; curl -fsSL https://antigravity.google/cli/install.sh | bash""#
    };
    assert!(built_in.contains(quoted), "{built_in}");
}
