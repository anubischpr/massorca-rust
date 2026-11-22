//! `massorca agent kinds`: built-in kinds, kinds declared in the profile, and declarations with problems.

mod harness;

use std::path::Path;

use harness::{Sandbox, stdout_text};
use predicates::str::contains;
use serde_json::{Value, json};

/// Writes `<profile>/agents/kinds/<file>`.
fn declare(sandbox: &Sandbox, file: &str, text: &str) {
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
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
