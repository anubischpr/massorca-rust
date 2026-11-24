//! `massorca agent-context`: the schema lists every core command and every local command, and a service that predates
//! the command table is refused with a clear error.

mod harness;

use std::collections::BTreeSet;

use harness::{Sandbox, single_document};
use massorca_cli::agent_context::{LOCAL_COMMANDS, require_capability};
use massorca_cli::error::exit;
use massorca_server::core_commands;
use predicates::str::{contains, is_empty};
use serde_json::Value;

/// The `result` of `--json agent-context` against the sandbox's default profile.
fn schema(sandbox: &Sandbox) -> Value {
    let outcome = sandbox
        .run("default", &["--json", "agent-context"])
        .success();
    let doc = single_document(&outcome);
    assert_eq!(doc["ok"], true, "{doc}");
    doc["result"].clone()
}

fn listed<'a>(result: &'a Value, path: &str) -> &'a Value {
    result["commands"]
        .as_array()
        .expect("commands is an array")
        .iter()
        .find(|command| command["command"] == path)
        .unwrap_or_else(|| panic!("`massorca {path}` is not listed"))
}

#[test]
fn json_document_holds_the_schema_version_the_count_and_one_object_per_command() {
    let sandbox = Sandbox::new();
    let result = schema(&sandbox);

    assert_eq!(result["schema_version"], 1);
    let commands = result["commands"].as_array().expect("commands is an array");
    assert_eq!(
        result["command_count"].as_u64(),
        Some(commands.len() as u64)
    );
    for command in commands {
        for key in [
            "command",
            "summary",
            "local",
            "method",
            "capability",
            "permission",
            "args_schema",
        ] {
            assert!(
                command.get(key).is_some(),
                "`{key}` is missing from {command}"
            );
        }
    }
}

#[test]
fn every_core_command_appears_with_its_method_capability_permission_and_parameters() {
    let sandbox = Sandbox::new();
    let result = schema(&sandbox);

    for descriptor in core_commands() {
        let path = descriptor.id.replace('.', " ");
        let command = listed(&result, &path);
        assert_eq!(command["local"], false, "`{path}` is a service command");
        assert_eq!(command["method"], descriptor.method, "`{path}`");
        assert_eq!(command["args_schema"], descriptor.args_schema, "`{path}`");
        assert!(
            command["capability"].is_string(),
            "`{path}` names its capability"
        );
        assert!(
            command["permission"].is_string(),
            "`{path}` names its permission"
        );
    }
}

#[test]
fn every_local_command_appears_marked_local_with_a_summary_and_no_request() {
    let sandbox = Sandbox::new();
    let result = schema(&sandbox);

    for local in LOCAL_COMMANDS {
        let path = local.replace('.', " ");
        let command = listed(&result, &path);
        assert_eq!(command["local"], true, "`{path}` runs on this computer");
        assert!(command["method"].is_null(), "`{path}` makes no request");
        assert!(
            !command["summary"].as_str().unwrap_or_default().is_empty(),
            "`{path}` has a summary"
        );
    }
}

#[test]
fn the_schema_lists_exactly_the_core_commands_and_the_local_ones() {
    let sandbox = Sandbox::new();
    let result = schema(&sandbox);

    let listed_paths: BTreeSet<String> = result["commands"]
        .as_array()
        .expect("commands is an array")
        .iter()
        .map(|command| command["command"].as_str().unwrap_or_default().to_owned())
        .collect();
    let expected: BTreeSet<String> = core_commands()
        .iter()
        .map(|descriptor| descriptor.id.replace('.', " "))
        .chain(LOCAL_COMMANDS.iter().map(|path| path.replace('.', " ")))
        .collect();
    assert_eq!(listed_paths, expected);
    assert_eq!(
        result["command_count"].as_u64(),
        Some(expected.len() as u64)
    );
}

#[test]
fn text_listing_names_each_command_and_writes_nothing_to_stderr() {
    let sandbox = Sandbox::new();

    sandbox
        .run("default", &["agent-context"])
        .success()
        .stdout(contains("git status"))
        .stdout(contains("host list"))
        .stderr(is_empty());
}

#[test]
fn a_service_that_does_not_offer_the_command_capability_gets_a_clear_error() {
    let older = vec!["service/1".to_owned(), "git/1".to_owned()];

    let error = require_capability(&older).expect_err("an older service is refused");

    assert_eq!(error.code, "service.capability_missing");
    assert_eq!(error.exit_code, exit::ERROR);
    assert!(error.message.contains("commands/1"), "{}", error.message);
    assert!(error.message.contains("restart"), "{}", error.message);
    let current = vec!["service/1".to_owned(), "commands/1".to_owned()];
    assert!(require_capability(&current).is_ok());
}
