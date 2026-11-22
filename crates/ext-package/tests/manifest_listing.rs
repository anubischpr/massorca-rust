//! The Marketplace listing: the item page data, the category rule, banner examples, connections and placements.

use massorca_ext_package::manifest::listing::{self, Category};
use massorca_ext_package::manifest::views::Slot;
use massorca_ext_package::{Error, Manifest, Problem, load, parse, validate};
use serde_json::{Value, json};

fn sticky_wall() -> Value {
    json!({
        "id": "massorca.sticky-wall",
        "name": "Sticky Wall",
        "version": "1.2.0",
        "publisher": "massorca",
        "description": "Notes on a wall, shared with your agents.",
        "api": "1.0",
        "requires": { "massorca": ">=0.6.0" },
        "views": 1,
        "surfaces": ["desktop", "android"],
        "modes": ["workspaces"],
        "main": "main.js",
        "permissions": [{ "kind": "storage", "reason": "Keeps your notes." }],
        "contributes": {
            "commands": [{ "id": "add-note", "title": "Add note", "handler": "addNote", "keys": "Ctrl+Alt+N" }],
            "views": [{ "id": "wall", "slot": "canvas-page", "title": "Wall", "document": "views/wall.json" }]
        }
    })
}

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

fn listing_with(value: Value) -> Value {
    let mut manifest = sticky_wall();
    manifest["listing"] = value;
    manifest
}

#[test]
fn a_valid_extension_loads_and_is_categorised_from_its_contributions() {
    let manifest = load(&bytes(&sticky_wall())).expect("valid manifest");
    assert_eq!(manifest.id, "massorca.sticky-wall");
    assert_eq!(manifest.api.to_string(), "1.0");
    assert_eq!(listing::category(&manifest), Category::Extensions);
    assert_eq!(listing::placements(&manifest), vec![Slot::CanvasPage]);
}

#[test]
fn a_data_only_theme_package_is_valid_and_categorised_as_themes() {
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
    let manifest = load(&bytes(&value)).expect("valid theme package");
    assert_eq!(listing::category(&manifest), Category::Themes);
}

#[test]
fn the_listing_logo_is_an_svg_or_png_package_path() {
    assert!(load(&bytes(&listing_with(json!({ "logo": "icon.svg" })))).is_ok());
    assert!(has_problem(
        &problems(&listing_with(json!({ "logo": "icon.gif" }))),
        "/listing/logo",
        "svg or .png"
    ));
}

#[test]
fn the_developer_url_must_be_https() {
    let ok = listing_with(
        json!({ "developer": { "name": "MassOrca extras", "url": "https://example.com" } }),
    );
    assert!(load(&bytes(&ok)).is_ok());
    let bad = listing_with(json!({ "developer": { "name": "Acme", "url": "http://example.com" } }));
    assert!(has_problem(
        &problems(&bad),
        "/listing/developer/url",
        "https URL"
    ));
}

#[test]
fn the_mention_has_no_at_sign_no_spaces_and_at_most_32_characters() {
    for mention in ["@GitHub", "Git Hub", ""] {
        let found = problems(&listing_with(json!({ "mention": mention })));
        assert!(
            has_problem(&found, "/listing/mention", "no @"),
            "{mention:?}"
        );
    }
    let long = "a".repeat(33);
    assert!(has_problem(
        &problems(&listing_with(json!({ "mention": long }))),
        "/listing/mention",
        "32"
    ));
    assert!(load(&bytes(&listing_with(json!({ "mention": "GitHub" })))).is_ok());
}

#[test]
fn a_declared_category_must_agree_with_the_contributions() {
    let skills = listing_with(json!({ "category": "skills" }));
    assert!(has_problem(
        &problems(&skills),
        "/listing/category",
        "needs a skills contribution"
    ));

    let agents = listing_with(json!({ "category": "agents" }));
    assert!(has_problem(
        &problems(&agents),
        "/listing/category",
        "needs an agentAdapters contribution"
    ));

    let services = listing_with(json!({ "category": "services" }));
    assert!(load(&bytes(&services)).is_ok());
}

#[test]
fn the_category_is_derived_when_absent() {
    let manifest = load(&bytes(&agent_package("acme", "acme-agent"))).expect("valid");
    assert_eq!(listing::category(&manifest), Category::Agents);

    let mut skills = sticky_wall();
    skills["contributes"] =
        json!({ "skills": [{ "id": "notes", "title": "Notes", "path": "skills/notes" }] });
    skills.as_object_mut().expect("object").remove("main");
    let manifest: Manifest = load(&bytes(&skills)).expect("valid");
    assert_eq!(listing::category(&manifest), Category::Skills);
}

#[test]
fn the_listing_object_and_the_old_icon_field_reject_unknown_fields() {
    let value = listing_with(json!({ "colour": "red" }));
    match parse(&bytes(&value)) {
        Err(Error::InvalidManifest(found)) => {
            assert!(found[0].message.contains("unknown field `colour`"))
        }
        other => panic!("expected a shape error, got {other:?}"),
    }
    let mut icon = sticky_wall();
    icon["icon"] = json!("icon.svg");
    match parse(&bytes(&icon)) {
        Err(Error::InvalidManifest(found)) => {
            assert!(found[0].message.contains("unknown field `icon`"))
        }
        other => panic!("expected a shape error, got {other:?}"),
    }
}
