//! Manifest core: the JSON shape, the identity, surfaces and modes, the main and readme files, and reporting every
//! problem in one pass.

use massorca_ext_package::manifest::check_files_present;
use massorca_ext_package::{Error, Manifest, Problem, load, parse, validate};
use serde_json::{Value, json};

/// A valid data-and-logic package with a `main` and one permission, and no contributions.
fn base() -> Value {
    json!({
        "id": "massorca.sticky-wall",
        "name": "Sticky Wall",
        "version": "1.2.0",
        "publisher": "massorca",
        "description": "Notes on a wall, shared with your agents.",
        "api": "1.0",
        "requires": { "massorca": ">=0.6.0" },
        "surfaces": ["desktop", "android"],
        "modes": ["workspaces"],
        "main": "main.js",
        "permissions": [{ "kind": "storage", "reason": "Keeps your notes." }]
    })
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("test JSON serializes")
}

/// Parses a shape-valid document and returns every semantic problem.
fn problems(value: &Value) -> Vec<Problem> {
    let manifest: Manifest = parse(&bytes(value)).expect("shape is valid for this case");
    validate(&manifest)
}

fn has(problems: &[Problem], path: &str, needle: &str) -> bool {
    problems
        .iter()
        .any(|problem| problem.path == path && problem.message.contains(needle))
}

/// The message of a shape error.
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
fn a_valid_package_loads_with_its_identity_and_api() {
    let manifest = load(&bytes(&base())).expect("valid manifest");
    assert_eq!(manifest.id, "massorca.sticky-wall");
    assert_eq!(manifest.publisher, "massorca");
    assert_eq!(manifest.api.to_string(), "1.0");
    assert_eq!(manifest.version.to_string(), "1.2.0");
}

#[test]
fn unknown_top_level_fields_are_rejected() {
    let mut value = base();
    value["bogus"] = json!(1);
    assert!(shape_message(&value).contains("unknown field `bogus`"));
}

#[test]
fn an_unknown_contribution_kind_is_rejected() {
    let mut value = base();
    value["contributes"] = json!({ "widgets": [] });
    assert!(shape_message(&value).contains("unknown field `widgets`"));
}

#[test]
fn an_unknown_permission_kind_is_rejected_through_the_manifest() {
    let mut value = base();
    value["permissions"] = json!([{ "kind": "root", "reason": "All of it." }]);
    assert!(shape_message(&value).contains("unknown permission kind `root`"));
}

#[test]
fn a_malformed_api_version_is_a_shape_error() {
    let mut value = base();
    value["api"] = json!("1");
    assert!(shape_message(&value).contains("is not major.minor"));
}

#[test]
fn a_version_that_is_not_semver_is_a_shape_error() {
    let mut value = base();
    value["version"] = json!("1.2");
    assert!(matches!(
        parse(&bytes(&value)),
        Err(Error::InvalidManifest(_))
    ));
}

#[test]
fn a_surface_outside_the_closed_pair_is_a_shape_error() {
    let mut value = base();
    value["surfaces"] = json!(["desktop", "web"]);
    assert!(matches!(
        parse(&bytes(&value)),
        Err(Error::InvalidManifest(_))
    ));
}

#[test]
fn trailing_data_after_the_document_is_refused() {
    let mut document = bytes(&base());
    document.extend_from_slice(b" trailing");
    assert!(matches!(parse(&document), Err(Error::InvalidManifest(_))));
}

#[test]
fn the_name_and_description_are_required() {
    let mut value = base();
    value["name"] = json!("   ");
    value["description"] = json!("");
    let found = problems(&value);
    assert!(has(&found, "/name", "is required"));
    assert!(has(&found, "/description", "is required"));
}

#[test]
fn the_id_must_be_publisher_dot_name_and_match_the_publisher() {
    let mut value = base();
    value["id"] = json!("Massorca.Sticky");
    assert!(has(&problems(&value), "/id", "publisher.name"));

    let mut value = base();
    value["publisher"] = json!("other");
    assert!(has(
        &problems(&value),
        "/publisher",
        "must equal the id prefix `massorca`"
    ));
}

#[test]
fn the_main_must_be_a_safe_wasm_or_js_path() {
    let mut value = base();
    value["main"] = json!("main.exe");
    assert!(has(&problems(&value), "/main", "must end in .wasm or .js"));

    let mut value = base();
    value["main"] = json!("../main.js");
    assert!(has(&problems(&value), "/main", "parent-directory segment"));
}

#[test]
fn the_readme_must_be_a_safe_package_path() {
    let mut value = base();
    value["readme"] = json!("/abs/README.md");
    assert!(has(&problems(&value), "/readme", "absolute path"));
}

#[test]
fn surfaces_and_modes_are_required_and_unique() {
    let mut value = base();
    value["surfaces"] = json!([]);
    value["modes"] = json!(["workspaces", "workspaces"]);
    let found = problems(&value);
    assert!(has(&found, "/surfaces", "at least one surface"));
    assert!(has(&found, "/modes/1", "listed twice"));

    let mut value = base();
    value["surfaces"] = json!(["desktop", "desktop"]);
    value["modes"] = json!(["Bot Teams"]);
    let found = problems(&value);
    assert!(has(&found, "/surfaces/1", "listed twice"));
    assert!(has(&found, "/modes/0", "not a valid mode name"));
}

#[test]
fn the_view_schema_major_starts_at_one() {
    let mut value = base();
    value["views"] = json!(0);
    assert!(has(&problems(&value), "/views", "starts at 1"));

    let mut value = base();
    value["views"] = json!(1);
    assert!(problems(&value).is_empty());
}

#[test]
fn every_problem_is_reported_in_one_pass() {
    let mut value = base();
    value["name"] = json!("");
    value["surfaces"] = json!(["desktop", "desktop"]);
    value["main"] = json!("main.exe");
    value["readme"] = json!("../README.md");
    let found = problems(&value);
    assert!(has(&found, "/name", "is required"));
    assert!(has(&found, "/surfaces/1", "listed twice"));
    assert!(has(&found, "/main", "must end in .wasm or .js"));
    assert!(has(&found, "/readme", "parent-directory segment"));
    assert_eq!(found.len(), 4);
}

#[test]
fn load_refuses_a_manifest_with_problems_and_lists_them() {
    let mut value = base();
    value["name"] = json!("");
    match load(&bytes(&value)) {
        Err(Error::InvalidManifest(found)) => {
            assert!(has(&found, "/name", "is required"));
            assert_eq!(
                Error::InvalidManifest(found).code(),
                "extension.invalid_manifest"
            );
        }
        other => panic!("expected invalid manifest, got {other:?}"),
    }
}

#[test]
fn the_main_file_must_be_in_the_package() {
    let manifest = load(&bytes(&base())).expect("valid manifest");
    assert!(check_files_present(&manifest, ["main.js"]).is_ok());
    match check_files_present(&manifest, Vec::<&str>::new()) {
        Err(Error::MissingFiles(missing)) => {
            assert_eq!(missing[0].path, "/main");
            assert_eq!(missing[0].message, "`main.js` is not in the package");
        }
        other => panic!("expected missing files, got {other:?}"),
    }
}
