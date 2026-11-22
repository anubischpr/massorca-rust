//! Logic contribution kinds: importers, events, tools, item sources and jobs. They run sandboxed code, so each needs a
//! `main` in the package.

use massorca_ext_package::manifest::check_files_present;
use massorca_ext_package::{Error, Manifest, Problem, load, parse, validate};
use serde_json::{Value, json};

/// A valid package with a `main`, so logic kinds are allowed. Contributions are added by each test.
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
fn logic_kinds_need_a_main() {
    let mut value = with(
        base(),
        json!({
            "importers": [{ "id": "ghostty", "title": "Ghostty", "handler": "importGhostty" }],
            "events": [{ "on": "workspace.created", "handler": "onWorkspace" }],
            "tools": [{ "id": "add_note", "description": "Add a note", "input": "schemas/add_note.json" }],
            "itemSources": [{ "id": "issues", "title": "Issues" }],
            "jobs": [{ "id": "tidy", "schedule": "daily", "handler": "tidy" }]
        }),
    );
    value.as_object_mut().expect("object").remove("main");
    let found = problems(&value);
    for kind in ["importers", "events", "tools", "itemSources", "jobs"] {
        assert!(
            has(
                &found,
                &format!("/contributes/{kind}"),
                "logic contributions need a main in the package"
            ),
            "{kind}"
        );
    }
}

#[test]
fn a_package_with_a_main_loads_every_logic_kind() {
    let value = with(
        base(),
        json!({
            "importers": [{ "id": "ghostty", "title": "Ghostty", "handler": "importGhostty" }],
            "events": [{ "on": "workspace.created", "handler": "onWorkspace" }],
            "tools": [{ "id": "add_note", "description": "Add a note", "input": "schemas/add_note.json" }],
            "itemSources": [{ "id": "issues", "title": "Issues" }],
            "jobs": [{ "id": "tidy", "schedule": "daily", "handler": "tidy" }]
        }),
    );
    let manifest = load(&bytes(&value)).expect("valid logic package");
    assert_eq!(manifest.contributes.events[0].on, "workspace.created");
    assert_eq!(manifest.contributes.jobs[0].schedule, "daily");
}

#[test]
fn event_names_are_dotted_lowercase() {
    let value = with(
        base(),
        json!({ "events": [
            { "on": "Workspace.Created", "handler": "onWorkspace" },
            { "on": "workspace..created", "handler": "onWorkspace" },
            { "on": "workspace.created", "handler": "onWorkspace" }
        ] }),
    );
    let found = problems(&value);
    assert!(has(
        &found,
        "/contributes/events/0/on",
        "not a dotted event name"
    ));
    assert!(has(
        &found,
        "/contributes/events/1/on",
        "not a dotted event name"
    ));
    assert!(
        !found
            .iter()
            .any(|problem| problem.path == "/contributes/events/2/on")
    );
}

#[test]
fn handlers_of_events_and_jobs_are_identifiers() {
    let value = with(
        base(),
        json!({
            "events": [{ "on": "workspace.created", "handler": "on-workspace" }],
            "jobs": [{ "id": "tidy", "schedule": "daily", "handler": "1tidy" }]
        }),
    );
    let found = problems(&value);
    assert!(has(
        &found,
        "/contributes/events/0/handler",
        "not a handler identifier"
    ));
    assert!(has(
        &found,
        "/contributes/jobs/0/handler",
        "not a handler identifier"
    ));
}

#[test]
fn a_job_needs_a_schedule() {
    let value = with(
        base(),
        json!({ "jobs": [{ "id": "tidy", "schedule": "", "handler": "tidy" }] }),
    );
    assert!(has(
        &problems(&value),
        "/contributes/jobs/0/schedule",
        "is required"
    ));
}

#[test]
fn a_tool_needs_a_description_and_a_safe_schema_file() {
    let value = with(
        base(),
        json!({ "tools": [{ "id": "add_note", "description": " ", "input": "/schemas/add_note.json" }] }),
    );
    let found = problems(&value);
    assert!(has(
        &found,
        "/contributes/tools/0/description",
        "is required"
    ));
    assert!(has(&found, "/contributes/tools/0/input", "absolute path"));
}

#[test]
fn a_tool_schema_must_be_in_the_package() {
    let value = with(
        base(),
        json!({ "tools": [{ "id": "add_note", "description": "Add a note", "input": "schemas/add_note.json" }] }),
    );
    let manifest = load(&bytes(&value)).expect("valid tool");
    assert!(check_files_present(&manifest, ["main.js", "schemas/add_note.json"]).is_ok());
    match check_files_present(&manifest, ["main.js"]) {
        Err(Error::MissingFiles(missing)) => {
            assert_eq!(missing[0].path, "/contributes/tools/0/input");
        }
        other => panic!("expected a missing schema, got {other:?}"),
    }
}

#[test]
fn logic_ids_follow_the_rule_and_are_unique_within_their_kind() {
    let value = with(
        base(),
        json!({
            "importers": [
                { "id": "ghostty", "title": "Ghostty", "handler": "importGhostty" },
                { "id": "ghostty", "title": "Again", "handler": "importGhostty" }
            ],
            "itemSources": [{ "id": "Issues", "title": "Issues" }]
        }),
    );
    let found = problems(&value);
    assert!(has(
        &found,
        "/contributes/importers/1/id",
        "declared twice in importers"
    ));
    assert!(has(
        &found,
        "/contributes/itemSources/0/id",
        "not a valid id"
    ));
}

#[test]
fn an_item_source_needs_a_title() {
    let value = with(
        base(),
        json!({ "itemSources": [{ "id": "issues", "title": "" }] }),
    );
    assert!(has(
        &problems(&value),
        "/contributes/itemSources/0/title",
        "is required"
    ));
}

#[test]
fn an_unknown_field_inside_an_event_is_rejected() {
    let value = with(
        base(),
        json!({ "events": [{ "on": "workspace.created", "handler": "onWorkspace", "priority": 1 }] }),
    );
    let message = match parse(&bytes(&value)) {
        Err(Error::InvalidManifest(found)) => found[0].message.clone(),
        other => panic!("expected a shape error, got {other:?}"),
    };
    assert!(message.contains("unknown field `priority`"));
}
