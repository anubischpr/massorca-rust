//! Risk computed from permissions and contributions, and the review an update needs.

use massorca_ext_package::permission::PermissionKind;
use massorca_ext_package::risk::{RiskLevel, assess};
use massorca_ext_package::update::review_update;
use massorca_ext_package::{Manifest, load};
use serde_json::{Value, json};

fn package(permissions: Value, contributes: Value) -> Value {
    json!({
        "id": "acme.tool-kit",
        "name": "Tool kit",
        "version": "1.0.0",
        "publisher": "acme",
        "description": "Tools.",
        "api": "1.0",
        "requires": { "massorca": ">=0.6.0" },
        "surfaces": ["desktop"],
        "modes": ["workspaces"],
        "main": "main.js",
        "permissions": permissions,
        "contributes": contributes
    })
}

fn manifest(value: &Value) -> Manifest {
    let mut value = value.clone();
    if value["contributes"]["agentAdapters"].is_array()
        || value["contributes"]["mcpServers"].is_array()
    {
        value.as_object_mut().expect("object").remove("main");
    }
    load(&serde_json::to_vec(&value).expect("serializes")).expect("valid manifest")
}

fn level_of(value: &Value) -> RiskLevel {
    assess(&manifest(value)).level
}

#[test]
fn a_package_with_only_low_permissions_is_low_with_no_reasons() {
    let value = package(
        json!([{ "kind": "storage", "reason": "Keeps notes." }]),
        json!({}),
    );
    let risk = assess(&manifest(&value));
    assert_eq!(risk.level, RiskLevel::Low);
    assert!(risk.reasons.is_empty());
}

#[test]
fn net_to_named_hosts_is_medium_and_to_any_host_is_high() {
    let named = package(
        json!([{ "kind": "net", "scope": { "hosts": ["api.example.com", "*.example.com"] }, "reason": "Sync." }]),
        json!({}),
    );
    assert_eq!(level_of(&named), RiskLevel::Medium);

    let any = package(
        json!([{ "kind": "net", "scope": { "hosts": ["*"] }, "reason": "Sync." }]),
        json!({}),
    );
    let risk = assess(&manifest(&any));
    assert_eq!(risk.level, RiskLevel::High);
    assert_eq!(risk.reasons[0].subject, "permission:net");

    let unscoped = package(json!([{ "kind": "net", "reason": "Sync." }]), json!({}));
    assert_eq!(level_of(&unscoped), RiskLevel::High);
}

#[test]
fn files_scoped_to_paths_are_medium_and_the_whole_root_is_high() {
    let scoped = package(
        json!([{ "kind": "files:write", "scope": { "paths": ["notes/**/*.md"] }, "reason": "Notes." }]),
        json!({}),
    );
    assert_eq!(level_of(&scoped), RiskLevel::Medium);

    let whole = package(
        json!([{ "kind": "files:write", "scope": { "paths": ["**"] }, "reason": "Everything." }]),
        json!({}),
    );
    assert_eq!(level_of(&whole), RiskLevel::High);
}

#[test]
fn process_and_credentials_are_always_high() {
    let process = package(
        json!([{ "kind": "process", "scope": { "programs": ["git status"] }, "reason": "Git." }]),
        json!({}),
    );
    assert_eq!(level_of(&process), RiskLevel::High);

    let credentials = package(
        json!([{ "kind": "credentials", "scope": { "agents": ["claude"] }, "reason": "Login." }]),
        json!({}),
    );
    assert_eq!(level_of(&credentials), RiskLevel::High);
}

#[test]
fn agent_facing_data_counts_toward_risk() {
    let skills = package(
        json!([]),
        json!({ "skills": [{ "id": "notes", "title": "Notes", "path": "skills/notes" }] }),
    );
    let risk = assess(&manifest(&skills));
    assert_eq!(risk.level, RiskLevel::High);
    assert_eq!(risk.reasons[0].subject, "contribution:skills");
    assert_eq!(
        risk.reasons[0].copy,
        "Instructs your agents, which act with your rights."
    );

    let recipes = package(
        json!([]),
        json!({ "recipes": [{ "id": "setup", "document": "recipes/setup.json" }] }),
    );
    assert_eq!(level_of(&recipes), RiskLevel::High);
}

#[test]
fn an_agent_package_is_high_and_names_the_adapter() {
    let value = json!({
        "id": "acme.agent-kit",
        "name": "Agent kit",
        "version": "0.1.0",
        "publisher": "acme",
        "description": "A coding agent.",
        "api": "1.0",
        "requires": { "massorca": ">=0.6.0" },
        "surfaces": ["desktop"],
        "modes": ["workspaces"],
        "permissions": [{ "kind": "process", "scope": { "programs": ["claude *"] }, "reason": "Runs the CLI." }],
        "contributes": {
            "agentAdapters": [{ "id": "acme-agent", "kind": "agents/acme-agent.json" }]
        }
    });
    let risk = assess(&manifest(&value));
    assert_eq!(risk.level, RiskLevel::High);
    assert!(
        risk.reasons
            .iter()
            .any(|reason| reason.subject == "agentAdapters:acme-agent")
    );
    assert!(
        risk.reasons
            .iter()
            .any(|reason| reason.subject == "permission:process")
    );
}

#[test]
fn an_adapter_with_an_install_list_adds_the_vendor_installer_reason() {
    let mut value = json!({
        "id": "massorca.claude",
        "name": "Claude",
        "version": "1.0.0",
        "publisher": "massorca",
        "description": "Claude Code.",
        "api": "1.0",
        "requires": { "massorca": ">=0.6.0" },
        "surfaces": ["desktop"],
        "modes": ["workspaces"],
        "permissions": [{ "kind": "process", "scope": { "programs": ["claude *"] }, "reason": "Runs the CLI." }],
        "contributes": {
            "agentAdapters": [{
                "id": "claude",
                "kind": "agents/claude.json",
                "cli": {
                    "binary": "claude",
                    "verify": ["claude", "--version"],
                    "source": "https://vendor.example/docs/install",
                    "install": [{ "os": "linux", "command": ["sh", "-c", "curl x | sh"] }]
                }
            }]
        }
    });
    let risk = assess(&manifest(&value));
    let install = risk
        .reasons
        .iter()
        .find(|reason| reason.subject == "agentAdapters:claude:install");
    assert_eq!(
        install.map(|reason| reason.copy),
        Some("Runs the vendor's installer on this computer.")
    );

    value["contributes"]["agentAdapters"][0]["cli"]
        .as_object_mut()
        .expect("object")
        .remove("install");
    let without = assess(&manifest(&value));
    assert!(
        without
            .reasons
            .iter()
            .all(|reason| reason.subject != "agentAdapters:claude:install")
    );
}

#[test]
fn an_update_that_adds_a_permission_requires_review() {
    let old = manifest(&package(
        json!([{ "kind": "storage", "reason": "Notes." }]),
        json!({}),
    ));
    let new_value = package(
        json!([
            { "kind": "storage", "reason": "Notes." },
            { "kind": "notifications", "reason": "Tells you when a note is due." }
        ]),
        json!({}),
    );
    let review = review_update(&old, &manifest(&new_value));
    assert_eq!(review.added_permissions.len(), 1);
    assert_eq!(
        review.added_permissions[0].kind,
        PermissionKind::Notifications
    );
    assert!(review.requires_review());
}

#[test]
fn an_update_that_widens_a_scope_lists_what_it_adds() {
    let old = manifest(&package(
        json!([{ "kind": "net", "scope": { "hosts": ["api.example.com"] }, "reason": "Sync." }]),
        json!({}),
    ));
    let new_value = package(
        json!([{ "kind": "net", "scope": { "hosts": ["api.example.com", "cdn.example.com"] }, "reason": "Sync." }]),
        json!({}),
    );
    let review = review_update(&old, &manifest(&new_value));
    assert_eq!(review.widened_permissions.len(), 1);
    assert_eq!(
        review.widened_permissions[0].added,
        vec!["cdn.example.com".to_owned()]
    );
    assert!(!review.widened_permissions[0].to_any);
    assert!(review.requires_review());
}

#[test]
fn an_update_that_drops_a_scope_to_any_is_a_widening() {
    let old = manifest(&package(
        json!([{ "kind": "secrets", "scope": { "names": ["token"] }, "reason": "Sign in." }]),
        json!({}),
    ));
    let new_value = package(
        json!([{ "kind": "secrets", "reason": "Sign in." }]),
        json!({}),
    );
    let review = review_update(&old, &manifest(&new_value));
    assert!(review.widened_permissions[0].to_any);
    assert!(review.requires_review());
}

#[test]
fn an_update_that_narrows_or_repeats_its_scope_needs_no_review() {
    let old = manifest(&package(
        json!([{ "kind": "net", "scope": { "hosts": ["api.example.com", "cdn.example.com"] }, "reason": "Sync." }]),
        json!({}),
    ));
    let narrower = package(
        json!([{ "kind": "net", "scope": { "hosts": ["api.example.com"] }, "reason": "Sync." }]),
        json!({}),
    );
    let review = review_update(&old, &manifest(&narrower));
    assert!(review.widened_permissions.is_empty());
    assert!(!review.requires_review());
}

#[test]
fn an_update_that_adds_a_skill_requires_review_even_without_new_permissions() {
    let old = manifest(&package(json!([]), json!({})));
    let new_value = package(
        json!([]),
        json!({ "skills": [{ "id": "notes", "title": "Notes", "path": "skills/notes" }] }),
    );
    let review = review_update(&old, &manifest(&new_value));
    assert!(review.added_permissions.is_empty());
    assert_eq!(review.added_reasons[0].subject, "contribution:skills");
    assert_eq!(review.risk_before, RiskLevel::Low);
    assert_eq!(review.risk_after, RiskLevel::High);
    assert!(review.requires_review());
}

#[test]
fn an_identical_update_needs_no_review() {
    let value = package(
        json!([{ "kind": "storage", "reason": "Notes." }]),
        json!({}),
    );
    let review = review_update(&manifest(&value), &manifest(&value));
    assert!(!review.requires_review());
}
