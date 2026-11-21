//! Terminals through the CLI: create, list, show, send, screen, resize, kill and close.

mod harness;

use harness::{Sandbox, single_document, stdout_text, with_json};
use predicates::str::contains;
use serde_json::Value;

/// Registers `<home>/notes` as a folder project and creates its workspace `main`; returns the workspace id.
fn folder_workspace(sandbox: &Sandbox) -> String {
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );
    let created = sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    created["id"].as_str().expect("workspace id").to_owned()
}

/// Creates a terminal in `main` with the default shell and returns its record.
fn create_terminal(sandbox: &Sandbox) -> Value {
    sandbox.json_ok("default", &["terminal", "create", "main"])
}

/// The `result` of `terminal show <id>`.
fn show(sandbox: &Sandbox, id: &str) -> Value {
    sandbox.json_ok("default", &["terminal", "show", id])
}

#[test]
fn a_terminal_is_created_in_a_workspace_listed_and_shown() {
    let sandbox = Sandbox::new();
    let workspace_id = folder_workspace(&sandbox);
    let workspace = sandbox.json_ok("default", &["workspace", "show", "main"]);

    let created = create_terminal(&sandbox);
    let id = created["id"].as_str().expect("terminal id").to_owned();
    assert_eq!(created["state"], "live");
    assert_eq!(created["workspace_id"], workspace_id);
    assert_eq!(
        created["cwd"], workspace["path"],
        "the shell starts in the workspace root"
    );
    assert!(
        created["shell"]
            .as_str()
            .is_some_and(|shell| !shell.is_empty()),
        "the default shell is named: {created}"
    );

    let listed = sandbox.json_ok("default", &["terminal", "list"]);
    assert_eq!(listed["terminals"].as_array().map(Vec::len), Some(1));
    let in_workspace = sandbox.json_ok("default", &["terminal", "list", "--workspace", "main"]);
    assert_eq!(in_workspace["terminals"][0]["id"], id);

    let shown = show(&sandbox, &id);
    assert_eq!(shown["id"], id);
    assert_eq!(shown["state"], "live");
    sandbox
        .run("default", &["terminal", "show", &id])
        .success()
        .stdout(contains(format!("id: {id}")))
        .stdout(contains("state: live"));
}

#[test]
fn the_json_envelope_holds_the_terminal_record_and_text_mode_has_no_envelope() {
    let sandbox = Sandbox::new();
    folder_workspace(&sandbox);

    let envelope = sandbox.run("default", &with_json(&["terminal", "create", "main"]));
    let document = single_document(&envelope.success());
    assert_eq!(document["ok"], true);
    assert_eq!(document["result"]["state"], "live");

    let text = sandbox.run("default", &["terminal", "list"]).success();
    assert!(
        !stdout_text(&text).contains("\"ok\""),
        "text mode prints no JSON envelope: {}",
        stdout_text(&text)
    );
}

#[test]
fn a_create_with_a_missing_directory_or_a_malformed_env_is_refused_before_anything_starts() {
    let sandbox = Sandbox::new();
    folder_workspace(&sandbox);

    sandbox
        .run(
            "default",
            &["terminal", "create", "main", "--cwd", "no-such-dir"],
        )
        .code(1)
        .stderr(contains("path.not_found"));
    let malformed = sandbox
        .run(
            "default",
            &["terminal", "create", "main", "--env", "SECRET_VALUE"],
        )
        .code(2);
    assert!(
        !stdout_text(&malformed).contains("SECRET_VALUE"),
        "the refused entry is not echoed"
    );
    sandbox
        .run("default", &["terminal", "list"])
        .success()
        .stdout(contains("no terminals"));
}
