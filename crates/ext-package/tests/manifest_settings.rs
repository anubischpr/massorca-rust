//! Settings: typed values, their defaults, and the section each appears in on the item page.

use massorca_ext_package::manifest::settings::{SettingSection, SettingType};
use massorca_ext_package::{Manifest, Problem, load, parse, validate};
use serde_json::{Value, json};

/// A valid package with a `main`, so handlers are allowed. Contributions are added by each test.
fn base() -> Value {
    json!({
        "id": "massorca.sticky-wall",
        "name": "Sticky Wall",
        "version": "1.2.0",
        "publisher": "massorca",
        "description": "Notes on a wall.",
        "api": "1.0",
        "requires": { "massorca": ">=0.6.0" },
        "surfaces": ["desktop"],
        "modes": ["workspaces"],
        "main": "main.js",
        "permissions": [{ "kind": "storage", "reason": "Keeps your notes." }]
    })
}

fn with(mut value: Value, contributes: Value) -> Value {
    value["contributes"] = contributes;
    value
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("test JSON serializes")
}

fn problems(value: &Value) -> Vec<Problem> {
    let manifest: Manifest = parse(&bytes(value)).expect("shape is valid for this case");
    validate(&manifest)
}

fn has(problems: &[Problem], path: &str, needle: &str) -> bool {
    problems
        .iter()
        .any(|problem| problem.path == path && problem.message.contains(needle))
}

#[test]
fn settings_defaults_must_match_their_type() {
    let value = with(
        base(),
        json!({ "settings": [
            { "key": "columns", "title": "Columns", "type": "integer", "default": "four" },
            { "key": "cache", "title": "Cache", "type": "boolean", "default": 1 },
            { "key": "scale", "title": "Scale", "type": "number", "default": 1.5 },
            { "key": "label", "title": "Label", "type": "string", "default": 3 }
        ] }),
    );
    let found = problems(&value);
    assert!(has(
        &found,
        "/contributes/settings/0/default",
        "must be an integer"
    ));
    assert!(has(
        &found,
        "/contributes/settings/1/default",
        "must be a boolean"
    ));
    assert!(has(
        &found,
        "/contributes/settings/3/default",
        "must be a string"
    ));
    assert_eq!(found.len(), 3);
}

#[test]
fn setting_keys_follow_the_key_rule_and_are_unique() {
    let value = with(
        base(),
        json!({ "settings": [
            { "key": "Bad Key", "title": "Bad", "type": "boolean", "default": true },
            { "key": "cache", "title": "Cache", "type": "boolean", "default": true },
            { "key": "cache", "title": "Again", "type": "boolean", "default": false }
        ] }),
    );
    let found = problems(&value);
    assert!(has(
        &found,
        "/contributes/settings/0/key",
        "not a valid key"
    ));
    assert!(has(
        &found,
        "/contributes/settings/2/key",
        "declared twice in settings"
    ));
}

#[test]
fn settings_default_to_the_basic_section_and_accept_advanced() {
    let value = with(
        base(),
        json!({ "settings": [
            { "key": "columns", "title": "Columns", "type": "integer", "default": 4 },
            { "key": "cache", "title": "Cache", "type": "boolean", "default": false, "section": "advanced" }
        ] }),
    );
    let manifest = load(&bytes(&value)).expect("valid settings");
    let settings = &manifest.contributes.settings;
    assert_eq!(settings[0].section, SettingSection::Basic);
    assert_eq!(settings[1].section, SettingSection::Advanced);
    assert_eq!(settings[1].value_type, SettingType::Boolean);
}
