//! `massorca agent kinds`: built-in kinds, kinds declared in the profile, and declarations with problems.

mod harness;

use std::path::Path;

use harness::{Sandbox, stdout_text};
use predicates::str::contains;
use serde_json::{Value, json};

/// Writes `<profile>/agents/kinds/<file>`.
fn declare(sandbox: &Sandbox, file: &str, text: &str) {
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join(file), text).expect("write a kind file");
}

/// A `fake` kind whose program is an existing file: this test's `massorca` binary.
fn fake_kind() -> Value {
    json!({
        "id": "fake",
        "name": "Fake agent",
        "icon": "bot",
        "program": [env!("CARGO_BIN_EXE_massorca")],
        "prompt": { "mode": "argv" },
        "status": { "mechanism": "env-command", "events": { "stop": "done" } }
    })
}

fn kind<'a>(kinds: &'a Value, id: &str, owner: &str) -> &'a Value {
    kinds["kinds"]
        .as_array()
        .expect("a kinds array")
        .iter()
        .find(|kind| kind["id"] == id && kind["owner"] == owner)
        .unwrap_or_else(|| panic!("no {owner} kind {id} in {kinds}"))
}

#[test]
fn built_in_kinds_are_listed_with_owner_core() {
    let sandbox = Sandbox::new();
    let kinds = sandbox.json_ok("default", &["agent", "kinds"]);
    let ids: Vec<&str> = kinds["kinds"]
        .as_array()
        .expect("a kinds array")
        .iter()
        .filter_map(|kind| kind["id"].as_str())
        .collect();
    assert_eq!(ids, ["claude", "codex", "antigravity", "gentle-shell"]);
    let gentle = kind(&kinds, "gentle-shell", "core");
    assert_eq!(gentle["name"], "Gentle Shell");
    assert_eq!(gentle["icon"], "rose");
    let claude = kind(&kinds, "claude", "core");
    assert_eq!(claude["status_mechanism"], "claude-settings");
    assert_eq!(claude["resumable"], true);
    assert_eq!(claude["credentials"]["supported"], true);
    assert_eq!(claude["credentials"]["imported"], false);
}

#[test]
fn a_profile_kind_with_an_existing_program_is_installed_with_owner_user() {
    let sandbox = Sandbox::new();
    declare(&sandbox, "fake.json", &fake_kind().to_string());

    let kinds = sandbox.json_ok("default", &["agent", "kinds"]);
    let fake = kind(&kinds, "fake", "user");
    assert_eq!(fake["installed"], true);
    assert_eq!(fake["name"], "Fake agent");
    assert_eq!(fake["status_mechanism"], "env-command");
    let path = fake["program_path"].as_str().expect("a program path");
    assert_eq!(
        Path::new(path),
        Path::new(env!("CARGO_BIN_EXE_massorca")),
        "the resolved path is the declared one"
    );
    assert!(fake.get("problems").is_none(), "no problems: {fake}");

    sandbox
        .run("default", &["agent", "kinds"])
        .success()
        .stdout(contains("fake"))
        .stdout(contains("Fake agent"))
        .stdout(contains("user"));
}

#[test]
fn invalid_files_and_built_in_ids_are_listed_with_problems() {
    let sandbox = Sandbox::new();
    declare(&sandbox, "broken.json", "{ \"id\": \"broken\", ");
    let shadow =
        json!({ "id": "claude", "name": "Shadow", "program": [env!("CARGO_BIN_EXE_massorca")] });
    declare(&sandbox, "shadow.json", &shadow.to_string());

    let kinds = sandbox.json_ok("default", &["agent", "kinds"]);
    let broken = kind(&kinds, "broken", "user");
    assert_eq!(broken["installed"], false);
    assert!(
        broken["problems"]
            .as_array()
            .is_some_and(|problems| !problems.is_empty()),
        "an unparsable file has a problem: {broken}"
    );
    let shadow = kind(&kinds, "claude", "user");
    assert_eq!(
        shadow["installed"], false,
        "a kind with problems is never installed"
    );
    assert!(
        shadow["problems"][0]
            .as_str()
            .is_some_and(|problem| problem.contains("built-in")),
        "{shadow}"
    );
    assert_eq!(kind(&kinds, "claude", "core")["name"], "Claude");

    let text = stdout_text(&sandbox.run("default", &["agent", "kinds"]).success());
    assert!(text.contains("problem: broken:"), "{text}");
    assert!(text.contains("invalid"), "{text}");
}

#[test]
fn an_edited_file_is_read_again_without_a_restart() {
    let sandbox = Sandbox::new();
    let before = sandbox.json_ok("default", &["agent", "kinds"]);
    assert_eq!(before["kinds"].as_array().map(Vec::len), Some(4));
    declare(&sandbox, "fake.json", &fake_kind().to_string());
    let after = sandbox.json_ok("default", &["agent", "kinds"]);
    assert_eq!(kind(&after, "fake", "user")["installed"], true);
}

/// The table row whose line holds `needle`, from the text output of `agent kinds`.
fn row<'a>(text: &'a str, needle: &str) -> &'a str {
    text.lines()
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| {
            panic!(
                "no row with {needle} in:
{text}"
            )
        })
}

#[test]
fn detection_and_package_state_are_separate_columns() {
    let sandbox = Sandbox::new();
    declare(&sandbox, "fake.json", &fake_kind().to_string());
    let mut absent = fake_kind();
    absent["id"] = json!("absent");
    absent["name"] = json!("Absent agent");
    absent["program"] = json!(["massorca-test-absent-agent"]);
    declare(&sandbox, "absent.json", &absent.to_string());

    let kinds = sandbox.json_ok("default", &["agent", "kinds"]);
    let fake = kind(&kinds, "fake", "user");
    assert_eq!(fake["detected"], true);
    assert_eq!(
        fake["installed"], true,
        "installed keeps its agents/1 meaning"
    );
    assert_eq!(fake["package"]["state"], "enabled");
    assert!(fake["package"].get("id").is_none(), "{fake}");
    let absent = kind(&kinds, "absent", "user");
    assert_eq!(absent["detected"], false);
    assert_eq!(absent["installed"], false);
    assert_eq!(absent["package"]["state"], "enabled");
    assert!(absent.get("program_path").is_none(), "{absent}");

    let text = stdout_text(&sandbox.run("default", &["agent", "kinds"]).success());
    let header = text.lines().next().expect("a header line");
    for column in ["INSTALLED", "DETECTED", "PATH"] {
        assert!(header.contains(column), "no {column} column in {header}");
    }
    let fake_row = row(&text, "Fake agent");
    assert!(fake_row.contains("enabled"), "{fake_row}");
    assert!(fake_row.contains("yes"), "{fake_row}");
    assert!(
        fake_row.contains(env!("CARGO_BIN_EXE_massorca")),
        "a detected kind shows its path: {fake_row}"
    );
    let absent_row = row(&text, "Absent agent");
    assert!(absent_row.contains("enabled"), "{absent_row}");
    assert!(absent_row.contains("no"), "{absent_row}");
    assert!(
        !absent_row.contains("massorca-test-absent-agent"),
        "a kind that does not resolve shows no path: {absent_row}"
    );
}

#[test]
fn kinds_list_their_permission_modes_with_the_default_and_bypass_marked() {
    let sandbox = Sandbox::new();
    let mut fake = fake_kind();
    fake["permission_modes"] = json!([
        { "id": "careful", "label": "Careful", "args": ["--mode", "careful"] },
        {
            "id": "wild", "label": "Wild", "args": ["--mode", "wild"], "bypass": true,
            "live_switch": { "type": "input", "text": "/mode wild" }
        }
    ]);
    fake["default_permission_mode"] = json!("careful");
    declare(&sandbox, "fake.json", &fake.to_string());

    let kinds = sandbox.json_ok("default", &["agent", "kinds"]);
    let listed = kind(&kinds, "fake", "user");
    assert_eq!(listed["default_permission_mode"], "careful");
    assert_eq!(
        listed["permission_modes"],
        json!([
            { "id": "careful", "label": "Careful", "bypass": false, "live_switch": false, "may_fall_back": false },
            { "id": "wild", "label": "Wild", "bypass": true, "live_switch": true, "may_fall_back": false }
        ]),
        "the modes are listed without their arguments"
    );
    let claude = kind(&kinds, "claude", "core");
    assert_eq!(
        claude["default_permission_mode"], "auto",
        "the vendor's own default"
    );

    let text = stdout_text(&sandbox.run("default", &["agent", "kinds"]).success());
    assert!(
        text.contains("modes: fake: careful (Careful, default), wild (Wild, bypass, live)"),
        "{text}"
    );
    assert!(
        text.contains("bypassPermissions (Bypass permissions, bypass)"),
        "{text}"
    );
    assert!(
        text.contains("auto (Auto, default, may fall back)"),
        "{text}"
    );
}

#[test]
fn a_kind_whose_default_mode_is_a_bypass_is_listed_with_the_problem() {
    let sandbox = Sandbox::new();
    let mut fake = fake_kind();
    fake["permission_modes"] =
        json!([{ "id": "wild", "label": "Wild", "args": ["--mode", "wild"], "bypass": true }]);
    fake["default_permission_mode"] = json!("wild");
    declare(&sandbox, "fake.json", &fake.to_string());
    let kinds = sandbox.json_ok("default", &["agent", "kinds"]);
    let listed = kind(&kinds, "fake", "user");
    assert_eq!(listed["detected"], false, "{listed}");
    assert!(
        listed["problems"][0]
            .as_str()
            .is_some_and(|problem| problem.contains("bypass mode")),
        "{listed}"
    );
}
