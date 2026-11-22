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
