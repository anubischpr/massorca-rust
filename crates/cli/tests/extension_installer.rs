//! Installing a built-in agent's CLI through the vendor commands its package declares, and removing it (0062). The
//! built-in catalogue is the only source of a vendor command, so each test runs a built-in, with the fake vendor standing
//! in for npm, winget, brew, powershell and bash. No real installer runs.

mod extension_support;
mod harness;

use std::fs;

use extension_support::Host;

const CODEX: &str = "catalogue:massorca.codex";
const ANTIGRAVITY: &str = "catalogue:massorca.antigravity";
const CLAUDE: &str = "catalogue:massorca.claude";
const GENTLE_SHELL: &str = "catalogue:massorca.gentle-shell";
const CLAUDE_ID: &str = "massorca.claude";

fn install_log(host: &Host, id: &str) -> String {
    fs::read_to_string(
        host.extensions_dir()
            .join("data")
            .join(id)
            .join("install.log"),
    )
    .unwrap_or_default()
}

#[test]
fn an_installer_needs_its_own_consent_and_nothing_runs_without_it() {
    let host = Host::new();
    host.run(&["extension", "install", CODEX]).code(1);
    let error = host.json_error(&["extension", "install", CODEX, "--yes"]);
    assert_eq!(error["code"], "extension.installer_consent_required");
    assert!(!host.has_cli("codex"), "the installer must not have run");
}

#[test]
fn with_both_flags_the_installer_runs_is_recorded_and_verified() {
    let host = Host::new();
    let record = host.json(&[
        "extension",
        "install",
        CODEX,
        "--yes",
        "--yes-run-installer",
    ]);
    let cli = &record["cli"][0];
    assert_eq!(cli["installed_by_package"], true);
    assert_eq!(cli["detected_before_install"], false);
    assert_eq!(cli["installer"]["exit_code"], 0);
    assert!(host.has_cli("codex"), "the CLI is on PATH");
    let log = install_log(&host, "massorca.codex");
    assert!(log.contains("@openai/codex"), "{log}");
    assert!(log.contains("fake-vendor: installed codex"), "{log}");
}

#[test]
fn a_failing_installer_leaves_nothing_installed() {
    let host = Host::new();
    fs::write(host.bin.path().join("fail-install"), "").expect("the install fails");
    let error = host.json_error(&[
        "extension",
        "install",
        CODEX,
        "--yes",
        "--yes-run-installer",
    ]);
    assert_eq!(error["code"], "extension.installer_failed");
    assert!(!host.has_cli("codex"));
    assert_eq!(
        host.json(&["extension", "list"])["extensions"],
        serde_json::json!([])
    );
}

#[test]
fn an_installer_that_leaves_no_cli_fails_the_verify_and_installs_nothing() {
    let host = Host::new();
    fs::write(host.bin.path().join("fail-verify"), "").expect("the verify fails");
    let error = host.json_error(&[
        "extension",
        "install",
        ANTIGRAVITY,
        "--yes",
        "--yes-run-installer",
    ]);
    assert_eq!(error["code"], "extension.verify_failed");
    assert_eq!(error["data"]["installer_ran"], true);
    assert_eq!(
        host.json(&["extension", "list"])["extensions"],
        serde_json::json!([])
    );
}

#[test]
fn removing_offers_the_uninstall_of_a_cli_the_package_installed() {
    let host = Host::new();
    host.json(&[
        "extension",
        "install",
        CLAUDE,
        "--yes",
        "--yes-run-installer",
    ]);

    let text = host.run(&["extension", "remove", CLAUDE_ID]).success();
    let output = String::from_utf8_lossy(&text.get_output().stdout).into_owned();

    assert!(output.contains("--uninstall-cli --yes"), "{output}");
    assert!(host.has_cli("claude"), "nothing was uninstalled");
}

#[test]
fn uninstall_cli_needs_its_confirmation_then_runs_the_uninstaller() {
    let host = Host::new();
    host.json(&[
        "extension",
        "install",
        CLAUDE,
        "--yes",
        "--yes-run-installer",
    ]);

    let refused = host.json_error(&["extension", "remove", CLAUDE_ID, "--uninstall-cli"]);
    assert_eq!(refused["code"], "extension.consent_required");
    assert!(refused["data"]["fingerprint"].is_string(), "{refused}");
    assert!(
        refused["message"]
            .as_str()
            .is_some_and(|message| message.ends_with(
                "This removes the CLI from this computer, so it stops working outside MassOrca too."
            )),
        "{refused}"
    );
    assert!(
        host.has_cli("claude"),
        "nothing was uninstalled without confirmation"
    );
    assert_eq!(
        host.json(&["extension", "list"])["extensions"][0]["id"],
        CLAUDE_ID,
        "the package is still installed"
    );

    let removed = host.json(&["extension", "remove", CLAUDE_ID, "--uninstall-cli", "--yes"]);
    assert_eq!(removed["cli"][0]["action"], "ran");
    assert!(!host.has_cli("claude"), "the uninstaller ran");
    assert!(
        host.extensions_dir()
            .join("logs/massorca.claude-uninstall.log")
            .exists(),
        "the uninstall is logged"
    );
}

#[test]
fn uninstall_cli_leaves_a_cli_the_package_did_not_install() {
    let host = Host::new();
    host.put_cli("claude");
    host.json(&["extension", "install", CLAUDE, "--yes"]);

    let removed = host.json(&["extension", "remove", CLAUDE_ID, "--uninstall-cli"]);

    assert_eq!(removed["cli"][0]["action"], "left_alone");
    assert!(
        host.has_cli("claude"),
        "the CLI was not the package's to remove"
    );
}

#[test]
fn codex_and_gentle_shell_uninstall_only_a_cli_they_installed() {
    for (source, id, binary) in [
        (CODEX, "massorca.codex", "codex"),
        (GENTLE_SHELL, "massorca.gentle-shell", "gentle-shell"),
    ] {
        let host = Host::new();
        host.json(&[
            "extension",
            "install",
            source,
            "--yes",
            "--yes-run-installer",
        ]);
        assert!(host.has_cli(binary), "{id} installed its CLI");

        let removed = host.json(&["extension", "remove", id, "--uninstall-cli", "--yes"]);
        assert_eq!(removed["cli"][0]["action"], "ran", "{removed}");
        assert!(!host.has_cli(binary), "{id} ran its uninstaller");
        assert!(
            host.extensions_dir()
                .join(format!("logs/{id}-uninstall.log"))
                .exists(),
            "{id} logged its uninstall"
        );

        let host = Host::new();
        host.put_cli(binary);
        host.json(&["extension", "install", source, "--yes"]);
        let removed = host.json(&["extension", "remove", id, "--uninstall-cli"]);
        assert_eq!(removed["cli"][0]["action"], "left_alone", "{removed}");
        assert!(host.has_cli(binary), "{id} kept a CLI it did not install");
    }
}

#[test]
fn an_agent_run_cannot_start_a_vendor_installer_or_uninstaller() {
    let host = Host::new();
    let agent = [
        ("MASSORCA_AGENT_SESSION", "session-1"),
        ("MASSORCA_AGENT_TOKEN", "not-a-real-token"),
    ];

    let install = host.json_error_with_env(
        &[
            "extension",
            "install",
            CODEX,
            "--consent",
            "unused",
            "--yes-run-installer",
        ],
        &agent,
    );
    assert_eq!(install["code"], "extension.needs_person");
    assert!(
        install["message"]
            .as_str()
            .is_some_and(|text| text.contains("ask the person to run the command")),
        "{install}"
    );

    let removal = host.json_error_with_env(
        &["extension", "remove", CLAUDE_ID, "--uninstall-cli"],
        &agent,
    );
    assert_eq!(removal["code"], "extension.needs_person");
    assert!(!host.has_cli("codex"), "nothing was installed");
}
