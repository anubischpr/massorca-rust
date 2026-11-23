//! The first-party agent packages, with the vendor commands 0062 records (checked 2026-11-22). Every test runs on the fake
//! host, so no vendor command or real installer runs.

use massorca_core_extensions::catalogue::entries;
use massorca_core_extensions::catalogue_agent_kinds;
use massorca_core_extensions::codes;
use massorca_core_extensions::source::Source;

use crate::fakes::FakeHost;
use crate::service::setup;

#[test]
fn antigravity_installer_exit_zero_with_a_failing_verify_registers_nothing() {
    // The installer exits 0 (an `irm | iex` failure can), and the CLI does not answer its verify command.
    let s = setup(FakeHost::new().exit_for("--version", 1));
    let source = Source::Catalogue("massorca.antigravity".to_owned());

    let error = s.install(&source, true).expect_err("verify is the gate");

    assert_eq!(error.code(), codes::VERIFY_FAILED);
    assert!(
        s.host
            .runs()
            .iter()
            .any(|argv| argv[0] == "powershell" || argv[0] == "bash"),
        "the installer ran: {:?}",
        s.host.runs()
    );
    assert!(s.lock_json().is_none(), "nothing is registered");
}

#[test]
fn codex_installer_exit_zero_with_no_binary_fails_because_nothing_verifies_it() {
    // Codex documents no verify command, so the binary resolving is the only check.
    let s = setup(FakeHost::new().without_binary());
    let source = Source::Catalogue("massorca.codex".to_owned());

    let error = s
        .install(&source, true)
        .expect_err("the binary does not resolve");

    assert_eq!(error.code(), codes::VERIFY_FAILED);
    assert_eq!(
        error.data().expect("the error says what ran")["command"],
        serde_json::json!(["npm", "install", "-g", "@openai/codex"])
    );
    assert!(s.lock_json().is_none(), "nothing is registered");
}

#[test]
fn gentle_shell_on_this_host_registers_without_running_anything() {
    let s = setup(FakeHost::new().with_program("gentle-shell"));
    let source = Source::Catalogue("massorca.gentle-shell".to_owned());

    s.install(&source, false)
        .expect("registers the detected CLI");

    assert!(s.host.runs().is_empty(), "nothing runs");
    let lock = s.lock_json().expect("the lockfile");
    assert_eq!(lock["packages"][0]["cli"][0]["installed_by_package"], false);
}

#[test]
fn gentle_shell_installs_with_npm_and_is_verified_by_its_version() {
    let s = setup(FakeHost::new());
    let source = Source::Catalogue("massorca.gentle-shell".to_owned());

    s.install(&source, true).expect("installs and verifies");

    assert_eq!(
        s.host.runs(),
        [
            vec!["npm", "install", "-g", "gentle-pi"],
            vec!["gentle-shell", "--version"],
        ]
    );
    let lock = s.lock_json().expect("the lockfile");
    assert_eq!(lock["packages"][0]["cli"][0]["installed_by_package"], true);
}

#[test]
fn gentle_shell_that_fails_its_version_check_is_not_registered() {
    // A host without the Pi runtime the package needs installs it but cannot run it.
    let s = setup(FakeHost::new().exit_for("--version", 1));
    let source = Source::Catalogue("massorca.gentle-shell".to_owned());

    let error = s.install(&source, true).expect_err("verify is the gate");

    assert_eq!(error.code(), codes::VERIFY_FAILED);
    assert!(s.lock_json().is_none(), "nothing is registered");
}

#[test]
fn the_gentle_shell_npm_name_appears_only_in_its_install_command() {
    let entry = entries()
        .iter()
        .find(|entry| entry.id == "massorca.gentle-shell")
        .expect("the built-in");
    for (path, bytes) in entry.files {
        let text = String::from_utf8_lossy(bytes);
        let outside_commands = text
            .lines()
            .filter(|line| !line.contains("\"command\""))
            .any(|line| line.contains("gentle-pi"));
        assert!(!outside_commands, "{path} names it outside a command");
    }
}

#[test]
fn each_kind_starts_the_binary_its_cli_block_names() {
    for kind in catalogue_agent_kinds() {
        let declaration: serde_json::Value =
            serde_json::from_slice(&kind.declaration).expect("a kind declaration");
        assert_eq!(
            declaration["program"][0], kind.cli.binary,
            "{}",
            kind.kind_id
        );
    }
}
