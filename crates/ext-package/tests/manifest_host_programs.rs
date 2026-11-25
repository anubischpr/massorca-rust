//! Host programs (`hostPrograms`, decision 0151): a tool package names a program on the host, how the vendor installs,
//! updates and uninstalls it, and how it is checked. The package never bundles the program.

use massorca_ext_package::manifest::listing::{Category, category};
use massorca_ext_package::risk::{RiskLevel, assess};
use massorca_ext_package::uninstall::uninstall_effects;
use massorca_ext_package::{Manifest, Problem, parse, validate};
use serde_json::{Value, json};

fn git_cli() -> Value {
    json!({
        "binary": "git", "verify": ["git", "--version"], "source": "https://git-scm.com/downloads",
        "install": [{ "os": "windows", "command": ["winget", "install", "--id", "Git.Git", "-e"] }],
        "update": [{ "os": "windows", "command": ["winget", "upgrade", "--id", "Git.Git", "-e"] }],
        "uninstall": [{ "os": "windows", "command": ["winget", "uninstall", "--id", "Git.Git", "-e"] }]
    })
}

fn tool_package(contributes: Value) -> Value {
    json!({
        "id": "massorca.git", "name": "Git", "version": "1.0.0", "publisher": "massorca",
        "description": "Version control.", "api": "1.0", "requires": { "massorca": ">=0.1.0" },
        "surfaces": ["desktop"], "modes": ["workspaces"],
        "permissions": [{ "kind": "process", "scope": { "programs": ["git"] }, "reason": "Runs Git." }],
        "contributes": contributes
    })
}

fn parsed(value: &Value) -> Manifest {
    parse(&serde_json::to_vec(value).expect("json")).expect("shape")
}

fn problems(value: &Value) -> Vec<Problem> {
    validate(&parsed(value))
}

#[test]
fn a_tool_package_is_valid_and_lists_under_tools() {
    let manifest = parsed(&tool_package(
        json!({ "hostPrograms": [{ "id": "git", "cli": git_cli() }] }),
    ));
    assert_eq!(validate(&manifest), Vec::new());
    assert_eq!(category(&manifest), Category::Tools);
    let programs: Vec<(&str, &str)> = manifest
        .program_clis()
        .map(|(id, cli)| (id, cli.binary.as_str()))
        .collect();
    assert_eq!(programs, [("git", "git")]);
    assert_eq!(manifest.cli_of("git").map(|cli| cli.update.len()), Some(1));
}

#[test]
fn host_programs_need_a_process_permission() {
    let mut value = tool_package(json!({ "hostPrograms": [{ "id": "git", "cli": git_cli() }] }));
    value["permissions"] = json!([]);
    assert!(
        problems(&value)
            .iter()
            .any(|problem| problem.path == "/contributes/hostPrograms"),
        "{:?}",
        problems(&value)
    );
}

#[test]
fn a_host_program_id_cannot_repeat_an_agent_id() {
    let value = tool_package(json!({
        "agentAdapters": [{ "id": "git", "kind": "agents/git.json" }],
        "hostPrograms": [{ "id": "git", "cli": git_cli() }]
    }));
    assert!(
        problems(&value)
            .iter()
            .any(|problem| problem.path == "/contributes/hostPrograms/0/id"),
        "{:?}",
        problems(&value)
    );
}

#[test]
fn an_update_step_must_start_with_an_allowlisted_program() {
    let mut cli = git_cli();
    cli["update"] = json!([{ "os": "linux", "command": ["apt-get", "upgrade", "git"] }]);
    let value = tool_package(json!({ "hostPrograms": [{ "id": "git", "cli": cli }] }));
    assert!(
        problems(&value)
            .iter()
            .any(|problem| problem.path == "/contributes/hostPrograms/0/cli/update/0/command"),
        "{:?}",
        problems(&value)
    );
}

#[test]
fn a_host_program_installer_is_high_risk() {
    let manifest = parsed(&tool_package(
        json!({ "hostPrograms": [{ "id": "git", "cli": git_cli() }] }),
    ));
    let risk = assess(&manifest);
    assert_eq!(risk.level, RiskLevel::High);
    assert!(
        risk.reasons
            .iter()
            .any(|reason| reason.subject == "hostPrograms:git:install"),
        "{:?}",
        risk.reasons
    );
}

#[test]
fn removal_names_the_program_only_when_the_package_installed_it() {
    let manifest = parsed(&tool_package(
        json!({ "hostPrograms": [{ "id": "git", "cli": git_cli() }] }),
    ));
    assert_eq!(uninstall_effects(&manifest, true).cli_binaries, ["git"]);
    assert!(uninstall_effects(&manifest, false).cli_binaries.is_empty());
    assert!(uninstall_effects(&manifest, true).offered.is_empty());
}
