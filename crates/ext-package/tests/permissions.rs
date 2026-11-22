//! The permission vocabulary: its shape, its scopes, and the rules a declared permission must meet.

use massorca_ext_package::Problem;
use massorca_ext_package::permission::{Permission, PermissionKind, PermissionScope, validate};
use serde_json::{Value, json};

fn parse(value: Value) -> Result<Permission, serde_json::Error> {
    serde_json::from_value(value)
}

/// Parses shape-valid permissions and returns their semantic problems under `/permissions`.
fn check(values: &[Value]) -> Vec<Problem> {
    let permissions: Vec<Permission> = values
        .iter()
        .map(|value| parse(value.clone()).expect("shape is valid for this case"))
        .collect();
    validate(&permissions, "/permissions")
}

fn has(problems: &[Problem], path: &str, needle: &str) -> bool {
    problems
        .iter()
        .any(|problem| problem.path == path && problem.message.contains(needle))
}

#[test]
fn every_kind_of_the_vocabulary_parses_by_its_spelling() {
    for kind in [
        "storage",
        "notifications",
        "workspaces:read",
        "files:read",
        "files:write",
        "net",
        "secrets",
        "terminals:read",
        "terminals:write",
        "agents:read",
        "agents:control",
        "sessions:read",
        "process",
        "credentials",
    ] {
        let permission = parse(json!({ "kind": kind, "reason": "Needed." }))
            .unwrap_or_else(|error| panic!("{kind}: {error}"));
        assert_eq!(permission.kind.wire(), kind);
    }
}

#[test]
fn an_unknown_kind_is_an_error_not_ignored() {
    let error = parse(json!({ "kind": "root", "reason": "All of it." })).expect_err("unknown kind");
    assert!(error.to_string().contains("unknown permission kind `root`"));
}

#[test]
fn a_kind_without_a_scope_refuses_one() {
    let error =
        parse(json!({ "kind": "storage", "scope": { "paths": ["a"] }, "reason": "Notes." }))
            .expect_err("storage takes no scope");
    assert!(error.to_string().contains("takes no scope"));
}

#[test]
fn the_process_scope_is_programs_and_the_old_commands_field_is_refused() {
    let error = parse(json!({
        "kind": "process",
        "scope": { "commands": ["git *"] },
        "reason": "Git."
    }))
    .expect_err("commands is no longer a scope field");
    assert!(error.to_string().contains("unknown field `commands`"));

    let permission = parse(json!({
        "kind": "process",
        "scope": { "programs": ["git status *"] },
        "reason": "Git."
    }))
    .expect("programs is the scope field");
    assert_eq!(
        permission.scope,
        Some(PermissionScope::Process(vec!["git status *".to_owned()]))
    );
}

#[test]
fn a_reason_is_required_by_the_shape() {
    let error = parse(json!({ "kind": "storage" })).expect_err("reason is required");
    assert!(error.to_string().contains("missing field `reason`"));
}

#[test]
fn a_blank_reason_is_a_problem() {
    let problems = check(&[json!({ "kind": "storage", "reason": "   " })]);
    assert!(has(
        &problems,
        "/permissions/0/reason",
        "a reason is required"
    ));
}

#[test]
fn a_kind_declared_twice_is_a_problem_on_the_second_entry() {
    let problems = check(&[
        json!({ "kind": "storage", "reason": "Notes." }),
        json!({ "kind": "storage", "reason": "More notes." }),
    ]);
    assert!(has(
        &problems,
        "/permissions/1/kind",
        "declared more than once"
    ));
}

#[test]
fn file_scopes_are_package_style_globs() {
    let problems = check(&[json!({
        "kind": "files:read",
        "scope": { "paths": ["notes/*.md", "../secrets"] },
        "reason": "Notes."
    })]);
    assert!(has(
        &problems,
        "/permissions/0/scope/paths/1",
        "not a valid paths entry"
    ));
    assert!(
        !problems
            .iter()
            .any(|problem| problem.path == "/permissions/0/scope/paths/0")
    );
}

#[test]
fn net_hosts_are_exact_wildcard_domains_or_any() {
    let problems = check(&[json!({
        "kind": "net",
        "scope": { "hosts": ["Api.example.com", "*.example.com", "*"] },
        "reason": "Sync."
    })]);
    assert!(has(
        &problems,
        "/permissions/0/scope/hosts/0",
        "not a valid hosts entry"
    ));
    assert_eq!(problems.len(), 1);
}

#[test]
fn secret_names_follow_the_name_rule() {
    let problems = check(&[json!({
        "kind": "secrets",
        "scope": { "names": ["-token", "example-token"] },
        "reason": "Sign in."
    })]);
    assert!(has(
        &problems,
        "/permissions/0/scope/names/0",
        "not a valid names entry"
    ));
    assert_eq!(problems.len(), 1);
}

#[test]
fn programs_are_program_plus_argument_patterns() {
    let problems = check(&[json!({
        "kind": "process",
        "scope": { "programs": ["", " git", "git status *"] },
        "reason": "Git."
    })]);
    assert!(has(
        &problems,
        "/permissions/0/scope/programs/0",
        "not a valid programs entry"
    ));
    assert!(has(
        &problems,
        "/permissions/0/scope/programs/1",
        "not a valid programs entry"
    ));
    assert_eq!(problems.len(), 2);
}

#[test]
fn an_empty_scope_list_is_a_problem() {
    let problems = check(&[json!({
        "kind": "credentials",
        "scope": { "agents": [] },
        "reason": "Login."
    })]);
    assert!(has(
        &problems,
        "/permissions/0/scope/agents",
        "lists at least one entry"
    ));
}

#[test]
fn credential_agents_are_kind_ids() {
    let problems = check(&[json!({
        "kind": "credentials",
        "scope": { "agents": ["claude", "Claude"] },
        "reason": "Login."
    })]);
    assert!(has(
        &problems,
        "/permissions/0/scope/agents/1",
        "not a valid agents entry"
    ));
    assert_eq!(problems.len(), 1);
}

#[test]
fn a_valid_list_has_no_problems() {
    let problems = check(&[
        json!({ "kind": "storage", "reason": "Notes." }),
        json!({ "kind": "process", "scope": { "programs": ["claude *"] }, "reason": "Runs the CLI." }),
    ]);
    assert!(problems.is_empty());
    assert_eq!(PermissionKind::Process.wire(), "process");
}
