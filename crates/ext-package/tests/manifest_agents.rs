//! Agent packages (`agentAdapters`) and MCP servers: the process tier. A package declares what to run and how its CLI
//! is obtained. It never bundles a vendor binary, and the install runs only after the user confirms.

use massorca_ext_package::{Error, Problem, load, parse, validate};
use serde_json::{Value, json};

fn agent_package(publisher: &str, id: &str) -> Value {
    json!({
        "id": format!("{publisher}.agent-kit"),
        "name": "Agent kit",
        "version": "0.1.0",
        "publisher": publisher,
        "description": "A coding agent.",
        "api": "1.0",
        "requires": { "massorca": ">=0.6.0" },
        "surfaces": ["desktop"],
        "modes": ["workspaces"],
        "permissions": [
            { "kind": "process", "scope": { "programs": ["claude *"] }, "reason": "Runs the agent CLI." }
        ],
        "contributes": {
            "agentAdapters": [{
                "id": id,
                "kind": format!("agents/{id}.json"),
                "cli": {
                    "binary": "claude",
                    "verify": ["claude", "--version"],
                    "source": "https://vendor.example/docs/install",
                    "install": [
                        { "os": "windows", "command": ["powershell", "-NoProfile", "-Command", "irm x | iex"] },
                        { "os": "linux", "command": ["sh", "-c", "curl x | sh"] }
                    ]
                }
            }]
        }
    })
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("test JSON serializes")
}

fn problems(value: &Value) -> Vec<Problem> {
    let manifest = parse(&bytes(value)).expect("shape is valid for this case");
    validate(&manifest)
}

fn has_problem(problems: &[Problem], path: &str, needle: &str) -> bool {
    problems
        .iter()
        .any(|problem| problem.path == path && problem.message.contains(needle))
}

fn shape_message(value: &Value) -> String {
    match parse(&bytes(value)) {
        Err(Error::InvalidManifest(found)) => found
            .iter()
            .map(|problem| problem.message.clone())
            .collect(),
        other => panic!("expected a shape error, got {other:?}"),
    }
}

#[test]
fn an_agent_package_with_process_permission_is_valid() {
    let manifest = load(&bytes(&agent_package("acme", "acme-agent"))).expect("valid agent package");
    assert!(manifest.main.is_none());
}

#[test]
fn an_agent_adapter_without_process_permission_is_rejected() {
    let mut value = agent_package("acme", "acme-agent");
    value["permissions"] = json!([]);
    let found = problems(&value);
    assert!(has_problem(
        &found,
        "/contributes/agentAdapters",
        "need a process permission"
    ));
}

#[test]
fn mcp_servers_need_process_permission_and_a_command() {
    let mut value = agent_package("acme", "acme-agent");
    value["permissions"] = json!([]);
    value["contributes"] = json!({ "mcpServers": [{ "id": "engram", "command": [] }] });
    let found = problems(&value);
    assert!(has_problem(
        &found,
        "/contributes/mcpServers",
        "need a process permission"
    ));
    assert!(has_problem(
        &found,
        "/contributes/mcpServers/0/command",
        "non-empty argv"
    ));
}

#[test]
fn the_reserved_agent_ids_belong_to_the_massorca_publisher() {
    let found = problems(&agent_package("acme", "claude"));
    assert!(has_problem(
        &found,
        "/contributes/agentAdapters/0/id",
        "reserved to publisher massorca"
    ));
    assert!(load(&bytes(&agent_package("massorca", "claude"))).is_ok());
}

#[test]
fn an_agent_kind_must_be_the_declaration_file_for_its_id() {
    let mut value = agent_package("acme", "acme-agent");
    value["contributes"]["agentAdapters"][0]["kind"] = json!("agents/other.json");
    let found = problems(&value);
    assert!(has_problem(
        &found,
        "/contributes/agentAdapters/0/kind",
        "must be exactly `agents/acme-agent.json`"
    ));
}

#[test]
fn agent_ids_follow_the_kind_id_rule() {
    let found = problems(&agent_package("acme", "Acme_Agent"));
    assert!(has_problem(
        &found,
        "/contributes/agentAdapters/0/id",
        "is not a valid id"
    ));
}

#[test]
fn the_cli_block_is_checked_field_by_field() {
    let cases: [(&str, Value, &str); 5] = [
        ("binary", json!("bin/claude"), "bare program name"),
        (
            "verify",
            json!(["other", "--version"]),
            "must start with the binary `claude`",
        ),
        (
            "source",
            json!("http://vendor.example/install"),
            "https URL",
        ),
        (
            "source",
            json!("https://user@vendor.example/install"),
            "https URL",
        ),
        (
            "install",
            json!([{ "os": "linux", "command": ["sh"] }, { "os": "linux", "command": ["sh"] }]),
            "one entry per operating system",
        ),
    ];
    for (field, replacement, needle) in cases {
        let mut value = agent_package("acme", "acme-agent");
        let cli = "/contributes/agentAdapters/0/cli";
        value["contributes"]["agentAdapters"][0]["cli"][field] = replacement;
        let found = problems(&value);
        assert!(
            found
                .iter()
                .any(|problem| problem.path.starts_with(cli) && problem.message.contains(needle)),
            "{field}: {found:?}"
        );
    }
}

#[test]
fn an_install_step_needs_a_non_empty_argv_and_os_names_are_closed() {
    let mut value = agent_package("acme", "acme-agent");
    value["contributes"]["agentAdapters"][0]["cli"]["install"] =
        json!([{ "os": "linux", "command": [] }]);
    let found = problems(&value);
    assert!(has_problem(
        &found,
        "/contributes/agentAdapters/0/cli/install/0/command",
        "non-empty argv"
    ));

    value["contributes"]["agentAdapters"][0]["cli"]["install"] =
        json!([{ "os": "freebsd", "command": ["sh"] }]);
    assert!(shape_message(&value).contains("unknown variant"));
}

#[test]
fn the_cli_block_is_optional_and_its_uninstall_list_defaults_to_empty() {
    let mut value = agent_package("acme", "acme-agent");
    value["contributes"]["agentAdapters"][0]
        .as_object_mut()
        .expect("object")
        .remove("cli");
    let manifest = load(&bytes(&value)).expect("valid without a CLI block");
    assert!(manifest.contributes.agent_adapters[0].cli.is_none());
}
