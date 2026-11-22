//! What removing an extension deletes, and what is only offered.

use massorca_ext_package::uninstall::{Offer, uninstall_effects};
use massorca_ext_package::{Manifest, load};
use serde_json::{Value, json};

fn manifest(value: &Value) -> Manifest {
    load(&serde_json::to_vec(value).expect("serializes")).expect("valid manifest")
}

fn agent_kit() -> Value {
    json!({
        "id": "massorca.claude",
        "name": "Claude",
        "version": "1.0.0",
        "publisher": "massorca",
        "description": "Claude Code.",
        "api": "1.0",
        "requires": { "massorca": ">=0.6.0" },
        "surfaces": ["desktop"],
        "modes": ["workspaces"],
        "permissions": [
            { "kind": "process", "scope": { "programs": ["claude *"] }, "reason": "Runs the CLI." },
            { "kind": "secrets", "scope": { "names": ["claude-token"] }, "reason": "Signs in." }
        ],
        "contributes": {
            "settings": [
                { "key": "model", "title": "Model", "type": "string", "default": "" },
                { "key": "cache", "title": "Cache", "type": "boolean", "default": true, "section": "advanced" }
            ],
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
        },
        "listing": {
            "connections": [{ "id": "account", "kind": "api-key", "title": "API key" }]
        }
    })
}

#[test]
fn removal_always_deletes_package_files_and_storage_and_every_setting() {
    let effects = uninstall_effects(&manifest(&agent_kit()), false);
    assert!(effects.package_files);
    assert!(effects.storage);
    assert_eq!(
        effects.settings,
        vec!["model".to_owned(), "cache".to_owned()]
    );
}

#[test]
fn secrets_are_named_by_their_scope_or_flagged_when_unscoped() {
    let effects = uninstall_effects(&manifest(&agent_kit()), false);
    assert_eq!(effects.secret_names, vec!["claude-token".to_owned()]);
    assert!(!effects.secrets_unscoped);

    let mut unscoped = agent_kit();
    unscoped["permissions"][1] = json!({ "kind": "secrets", "reason": "Signs in." });
    let effects = uninstall_effects(&manifest(&unscoped), false);
    assert!(effects.secret_names.is_empty());
    assert!(effects.secrets_unscoped);
}

#[test]
fn connections_are_listed_for_their_stored_sign_in() {
    let effects = uninstall_effects(&manifest(&agent_kit()), false);
    assert_eq!(effects.connections, vec!["account".to_owned()]);
}

#[test]
fn the_cli_is_removed_only_when_the_package_installed_it() {
    let kept = uninstall_effects(&manifest(&agent_kit()), false);
    assert!(kept.cli_binaries.is_empty());

    let installed = uninstall_effects(&manifest(&agent_kit()), true);
    assert_eq!(installed.cli_binaries, vec!["claude".to_owned()]);
}

#[test]
fn the_agent_credential_home_is_offered_not_deleted() {
    let effects = uninstall_effects(&manifest(&agent_kit()), false);
    assert_eq!(
        effects.offered,
        vec![Offer::AgentCredentialHome {
            agent: "claude".to_owned()
        }]
    );
}

#[test]
fn a_package_without_listing_or_secrets_reports_only_what_it_has() {
    let value = json!({
        "id": "acme.night-theme",
        "name": "Night",
        "version": "1.0.0",
        "publisher": "acme",
        "description": "A dark theme.",
        "api": "1.0",
        "requires": { "massorca": ">=0.6.0" },
        "surfaces": ["desktop"],
        "modes": ["workspaces"],
        "contributes": { "themes": [{ "id": "night", "title": "Night", "document": "themes/night.json" }] }
    });
    let effects = uninstall_effects(&manifest(&value), true);
    assert!(effects.package_files && effects.storage);
    assert!(effects.settings.is_empty());
    assert!(effects.secret_names.is_empty() && !effects.secrets_unscoped);
    assert!(effects.connections.is_empty());
    assert!(effects.cli_binaries.is_empty());
    assert!(effects.offered.is_empty());
}
