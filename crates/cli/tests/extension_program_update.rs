//! Tool packages from the CLI (decision 0151): Git installed, updated and removed through the fake vendor standing in for
//! winget and brew, in a throwaway profile. No real installer, update or uninstall runs. Linux tool packages declare no
//! command, so these run on Windows and macOS only.
#![cfg(any(windows, target_os = "macos"))]

mod extension_support;
mod harness;

use std::fs;

use extension_support::Host;

const GIT: &str = "catalogue:massorca.git";
const GIT_ID: &str = "massorca.git";

fn install_log(host: &Host) -> String {
    fs::read_to_string(
        host.extensions_dir()
            .join("data")
            .join(GIT_ID)
            .join("install.log"),
    )
    .unwrap_or_default()
}

#[test]
fn git_installs_and_updates_only_after_the_update_review_is_accepted() {
    let host = Host::new();
    let record = host.json(&["extension", "install", GIT, "--yes", "--yes-run-installer"]);
    assert_eq!(record["cli"][0]["installed_by_package"], true);
    assert!(host.has_cli("git"));

    let refused = host.json_error(&["extension", "program-update", GIT_ID, "git"]);
    assert_eq!(refused["code"], "extension.consent_required");
    assert!(!install_log(&host).contains("updated git"), "nothing ran");

    let updated = host.json(&["extension", "program-update", GIT_ID, "git", "--yes"]);
    assert_eq!(updated["exit_code"], 0);
    assert!(
        install_log(&host).contains("fake-vendor: updated git"),
        "{}",
        install_log(&host)
    );
}

#[test]
fn a_git_massorca_did_not_install_is_never_updated_or_uninstalled() {
    let host = Host::new();
    host.put_cli("git");
    let record = host.json(&["extension", "install", GIT, "--yes"]);
    assert_eq!(record["cli"][0]["installed_by_package"], false);

    let refused = host.json_error(&["extension", "program-update", GIT_ID, "git", "--yes"]);
    assert_eq!(refused["code"], "extension.program_update_refused");

    let removed = host.json(&["extension", "remove", GIT_ID, "--uninstall-cli", "--yes"]);
    assert_eq!(removed["cli"][0]["action"], "left_alone");
    assert!(host.has_cli("git"), "the user's Git stays");
}

#[test]
fn doctor_reports_where_a_tool_package_finds_its_program() {
    let host = Host::new();
    host.put_cli("git");
    let output = host.run(&["extension", "doctor", GIT_ID]).success();
    let text = String::from_utf8_lossy(&output.get_output().stdout).into_owned();
    assert!(text.contains("program git: git found"), "{text}");
}
