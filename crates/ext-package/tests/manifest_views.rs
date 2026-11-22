//! Views: view documents placed in a UI slot, and the view-schema major they need.

use massorca_ext_package::manifest::views::Slot;
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
        "views": 1,
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
fn a_view_names_a_slot_from_the_closed_list_and_a_safe_document() {
    let value = with(
        base(),
        json!({ "views": [{ "id": "wall", "slot": "canvas-page", "title": "Wall", "document": "views/wall.json" }] }),
    );
    let manifest = load(&bytes(&value)).expect("valid view");
    assert_eq!(manifest.contributes.views[0].slot, Slot::CanvasPage);

    let value = with(
        base(),
        json!({ "views": [{ "id": "wall", "slot": "sidebar", "title": "Wall", "document": "views/wall.json" }] }),
    );
    assert!(shape_message(&value).contains("unknown variant"));

    let value = with(
        base(),
        json!({ "views": [{ "id": "wall", "slot": "canvas-page", "title": "Wall", "document": "../wall.json" }] }),
    );
    assert!(has(
        &problems(&value),
        "/contributes/views/0/document",
        "parent-directory segment"
    ));
}

#[test]
fn contributed_views_need_the_view_schema_major() {
    let mut value = with(
        base(),
        json!({ "views": [{ "id": "wall", "slot": "canvas-page", "title": "Wall", "document": "views/wall.json" }] }),
    );
    value.as_object_mut().expect("object").remove("views");
    assert!(has(
        &problems(&value),
        "/views",
        "is required when views are contributed"
    ));

    value["views"] = json!(1);
    assert!(problems(&value).is_empty());
}
