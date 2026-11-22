//! Manifest text the review prints must not hide or reorder characters, and a publisher or id segment must not name a
//! Windows device.

use massorca_ext_package::{Manifest, Problem, parse, validate};
use serde_json::{Value, json};

fn base() -> Value {
    json!({
        "id": "acme.kit", "name": "Kit", "version": "1.0.0", "publisher": "acme",
        "description": "A kit.", "api": "1.0", "requires": { "massorca": ">=0.1.0" },
        "surfaces": ["desktop"], "modes": ["workspaces"],
        "permissions": [{ "kind": "storage", "reason": "Keeps notes." }]
    })
}

fn problems(value: &Value) -> Vec<Problem> {
    let manifest: Manifest = parse(&serde_json::to_vec(value).expect("json")).expect("shape");
    validate(&manifest)
}

fn refused_at(value: &Value, path: &str) -> bool {
    problems(value).iter().any(|problem| problem.path == path)
}

#[test]
fn a_manifest_without_hidden_characters_is_valid() {
    assert!(problems(&base()).is_empty(), "{:?}", problems(&base()));
}

#[test]
fn a_bidi_override_in_the_name_description_or_reason_is_refused() {
    let mut name = base();
    name["name"] = json!("Kit\u{202E}gpj.exe");
    assert!(refused_at(&name, "/name"));

    let mut description = base();
    description["description"] = json!("A\u{2066}kit");
    assert!(refused_at(&description, "/description"));

    let mut reason = base();
    reason["permissions"][0]["reason"] = json!("Keeps\u{200B} notes.");
    assert!(refused_at(&reason, "/permissions/0/reason"));
}

#[test]
fn a_format_character_in_an_argv_part_is_refused() {
    let mut value = base();
    value["contributes"] = json!({ "agentAdapters": [{
        "id": "acme", "kind": "agents/acme.json",
        "cli": {
            "binary": "acme", "verify": ["acme", "--version"],
            "source": "https://vendor.example/install",
            "install": [{ "os": "linux", "command": ["npm", "install\u{FEFF}"] }]
        }
    }] });
    assert!(refused_at(
        &value,
        "/contributes/agentAdapters/0/cli/install/0/command"
    ));
}

#[test]
fn a_device_name_as_publisher_is_refused_by_the_id_rule() {
    let mut value = base();
    value["id"] = json!("con.kit");
    value["publisher"] = json!("con");
    assert!(refused_at(&value, "/id"));
}
