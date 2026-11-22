//! Data contribution kinds: commands, keybindings, themes, documents and skills.

use massorca_ext_package::manifest::check_files_present;
use massorca_ext_package::{Error, Manifest, Problem, load, parse, validate};
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

/// The same package without a `main`: data only.
fn data_only() -> Value {
    let mut value = base();
    value.as_object_mut().expect("object").remove("main");
    value
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
fn a_data_only_package_with_themes_loads() {
    let value = with(
        data_only(),
        json!({ "themes": [{ "id": "night", "title": "Night", "document": "themes/night.json" }] }),
    );
    let manifest = load(&bytes(&value)).expect("valid data-only package");
    assert_eq!(manifest.contributes.themes[0].id, "night");
}

#[test]
fn a_command_without_a_handler_is_data_and_needs_no_main() {
    let value = with(
        data_only(),
        json!({ "commands": [{ "id": "add-note", "title": "Add note", "keys": "Ctrl+Alt+N" }] }),
    );
    assert!(load(&bytes(&value)).is_ok());
}

#[test]
fn a_command_handler_needs_a_main() {
    let value = with(
        data_only(),
        json!({ "commands": [{ "id": "add-note", "title": "Add note", "handler": "addNote" }] }),
    );
    assert!(has(
        &problems(&value),
        "/contributes/commands/0/handler",
        "needs a main in the package"
    ));
}

#[test]
fn a_handler_must_be_an_identifier() {
    let value = with(
        base(),
        json!({ "commands": [{ "id": "add-note", "title": "Add note", "handler": "add-note" }] }),
    );
    assert!(has(
        &problems(&value),
        "/contributes/commands/0/handler",
        "not a handler identifier"
    ));
}

#[test]
fn command_ids_follow_the_rule_and_are_unique_within_commands() {
    let value = with(
        base(),
        json!({ "commands": [
            { "id": "Add Note", "title": "One" },
            { "id": "add-note", "title": "Two" },
            { "id": "add-note", "title": "Three" }
        ] }),
    );
    let found = problems(&value);
    assert!(has(&found, "/contributes/commands/0/id", "not a valid id"));
    assert!(has(
        &found,
        "/contributes/commands/2/id",
        "declared twice in commands"
    ));
}

#[test]
fn a_command_needs_a_title_and_non_empty_keys_when_it_names_them() {
    let value = with(
        base(),
        json!({ "commands": [{ "id": "add-note", "title": " ", "keys": "" }] }),
    );
    let found = problems(&value);
    assert!(has(&found, "/contributes/commands/0/title", "is required"));
    assert!(has(&found, "/contributes/commands/0/keys", "is required"));
}

#[test]
fn an_unknown_field_inside_a_command_is_rejected() {
    let value = with(
        base(),
        json!({ "commands": [{ "id": "add-note", "title": "Add", "hotkey": "x" }] }),
    );
    assert!(shape_message(&value).contains("unknown field `hotkey`"));
}

#[test]
fn keybindings_need_keys_and_a_command() {
    let value = with(
        base(),
        json!({ "keybindings": [{ "keys": "", "command": "" }] }),
    );
    let found = problems(&value);
    assert!(has(
        &found,
        "/contributes/keybindings/0/keys",
        "is required"
    ));
    assert!(has(
        &found,
        "/contributes/keybindings/0/command",
        "is required"
    ));
}

#[test]
fn theme_documents_are_safe_package_paths() {
    let value = with(
        base(),
        json!({
            "themes": [{ "id": "night", "title": "Night", "document": "../night.json" }],
            "iconThemes": [{ "id": "dots", "title": "Dots", "document": "C:/dots.json" }]
        }),
    );
    let found = problems(&value);
    assert!(has(
        &found,
        "/contributes/themes/0/document",
        "parent-directory segment"
    ));
    assert!(has(
        &found,
        "/contributes/iconThemes/0/document",
        "drive letter"
    ));
}

#[test]
fn locales_recipes_and_agent_rules_name_safe_documents() {
    let value = with(
        base(),
        json!({
            "locales": [{ "id": "fr", "document": "locales/../fr.json" }],
            "recipes": [{ "id": "setup", "document": "recipes\\setup.json" }],
            "agentRules": [{ "id": "quiet", "document": "/rules/quiet.json" }]
        }),
    );
    let found = problems(&value);
    assert!(has(
        &found,
        "/contributes/locales/0/document",
        "parent-directory segment"
    ));
    assert!(has(
        &found,
        "/contributes/recipes/0/document",
        "backslash separator"
    ));
    assert!(has(
        &found,
        "/contributes/agentRules/0/document",
        "absolute path"
    ));
}

#[test]
fn a_skill_is_a_folder_and_needs_its_skill_md_in_the_package() {
    let value = with(
        base(),
        json!({ "skills": [{ "id": "notes", "title": "Notes", "path": "skills/notes/" }] }),
    );
    assert!(has(
        &problems(&value),
        "/contributes/skills/0/path",
        "empty segment"
    ));

    let value = with(
        base(),
        json!({ "skills": [{ "id": "notes", "title": "Notes", "path": "skills/notes" }] }),
    );
    let manifest = load(&bytes(&value)).expect("valid skill");
    assert!(check_files_present(&manifest, ["main.js", "skills/notes/SKILL.md"]).is_ok());
    match check_files_present(&manifest, ["main.js"]) {
        Err(Error::MissingFiles(missing)) => {
            assert_eq!(missing[0].path, "/contributes/skills/0/path");
            assert_eq!(
                missing[0].message,
                "`skills/notes/SKILL.md` is not in the package"
            );
        }
        other => panic!("expected missing SKILL.md, got {other:?}"),
    }
}

#[test]
fn a_contributed_document_must_be_in_the_package() {
    let value = with(
        base(),
        json!({ "themes": [{ "id": "night", "title": "Night", "document": "themes/night.json" }] }),
    );
    let manifest = load(&bytes(&value)).expect("valid theme");
    match check_files_present(&manifest, ["main.js"]) {
        Err(Error::MissingFiles(missing)) => {
            assert_eq!(missing[0].path, "/contributes/themes/0/document");
        }
        other => panic!("expected a missing theme file, got {other:?}"),
    }
}
