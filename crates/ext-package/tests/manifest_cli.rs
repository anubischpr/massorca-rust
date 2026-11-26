//! The `cli` block of an agent adapter: `verify` is optional, and `sourceChecked` is a date when given (0062).

use massorca_ext_package::{Manifest, Problem, parse, validate};
use serde_json::{Value, json};

fn package(cli: Value) -> Value {
    json!({
        "id": "massorca.codex", "name": "Codex", "version": "1.0.0", "publisher": "massorca",
        "description": "A coding agent.", "api": "1.0", "requires": { "massorca": ">=0.1.0" },
        "surfaces": ["desktop"], "modes": ["workspaces"],
        "permissions": [{ "kind": "process", "scope": { "programs": ["codex"] }, "reason": "Runs it." }],
        "contributes": { "agentAdapters": [{ "id": "codex", "kind": "agents/codex.json", "cli": cli }] }
    })
}

fn problems(cli: Value) -> Vec<Problem> {
    let manifest: Manifest =
        parse(&serde_json::to_vec(&package(cli)).expect("json")).expect("shape");
    validate(&manifest)
}

#[test]
fn a_cli_without_verify_is_valid() {
    let cli = json!({
        "binary": "codex", "source": "https://github.com/openai/codex", "sourceChecked": "2026-11-22",
        "install": [{ "os": "linux", "command": ["npm", "install", "-g", "@openai/codex"] }]
    });
    assert!(problems(cli).is_empty());
}

#[test]
fn a_verify_that_does_not_start_with_the_binary_is_refused() {
    let cli = json!({ "binary": "codex", "verify": ["other", "--version"], "source": "https://vendor.example/x" });
    assert!(
        problems(cli)
            .iter()
            .any(|problem| problem.path.ends_with("/verify"))
    );
}

#[test]
fn source_checked_must_be_a_real_date() {
    for good in ["2026-11-22", "2024-02-29"] {
        let cli = json!({ "binary": "codex", "source": "https://vendor.example/x", "sourceChecked": good });
        assert!(problems(cli).is_empty(), "{good}");
    }
    for bad in [
        "2026-13-01",
        "2026-11-00",
        "2026-11-32",
        "26-11-22",
        "2026/11/22",
        "2026-11-2x",
        "",
    ] {
        let cli = json!({ "binary": "codex", "source": "https://vendor.example/x", "sourceChecked": bad });
        assert!(
            problems(cli)
                .iter()
                .any(|problem| problem.path.ends_with("/sourceChecked")),
            "{bad}"
        );
    }
}

#[test]
fn an_install_program_must_be_on_the_vendor_allowlist_and_bare() {
    for program in ["sh", "curl", "/usr/bin/npm", "./npm", "C:/Tools/npm.exe"] {
        let cli = json!({
            "binary": "codex", "source": "https://vendor.example/x",
            "install": [{ "os": "linux", "command": [program, "install"] }]
        });
        assert!(
            problems(cli)
                .iter()
                .any(|problem| problem.path.ends_with("/install/0/command")),
            "{program} is refused"
        );
    }
    let allowed = json!({
        "binary": "codex", "source": "https://vendor.example/x",
        "install": [{ "os": "linux", "command": ["npm", "install", "-g", "@openai/codex"] }]
    });
    assert!(problems(allowed).is_empty());
}

#[test]
fn an_uninstall_may_start_with_the_clis_own_binary_but_an_install_may_not() {
    let own_uninstall = json!({
        "binary": "gentle-shell", "source": "https://github.com/vendor/gentle-shell",
        "uninstall": [{ "os": "linux", "command": ["gentle-shell", "self-uninstall", "--yes"] }]
    });
    assert!(problems(own_uninstall).is_empty());
    let own_install = json!({
        "binary": "gentle-shell", "source": "https://github.com/vendor/gentle-shell",
        "install": [{ "os": "linux", "command": ["gentle-shell", "install"] }]
    });
    assert!(
        problems(own_install)
            .iter()
            .any(|problem| problem.path.ends_with("/install/0/command")),
        "an install never starts the CLI itself"
    );
    let other_uninstall = json!({
        "binary": "gentle-shell", "source": "https://github.com/vendor/gentle-shell",
        "uninstall": [{ "os": "linux", "command": ["gentle-tools", "self-uninstall"] }]
    });
    assert!(
        problems(other_uninstall)
            .iter()
            .any(|problem| problem.path.ends_with("/uninstall/0/command")),
        "an uninstall starts only the CLI's own binary or an allowlisted program"
    );
}
