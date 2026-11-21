//! Terminals through the CLI: create, list, show, send, screen, resize, kill and close.

mod harness;

use harness::{Sandbox, eventually, single_document, stdout_text, with_json};
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
fn text_sent_to_a_terminal_appears_on_its_screen() {
    let sandbox = Sandbox::new();
    folder_workspace(&sandbox);
    let id = create_terminal(&sandbox)["id"]
        .as_str()
        .expect("terminal id")
        .to_owned();

    sandbox
        .run(
            "default",
            &["terminal", "send", &id, "echo", "terminal-test-marker"],
        )
        .success();
    // The shell echoes the command and then prints the marker alone on its own line.
    let screen = eventually("the marker on the screen", || {
        let text = stdout_text(
            &sandbox
                .run("default", &["terminal", "screen", &id])
                .success(),
        );
        text.lines()
            .any(|line| line.trim() == "terminal-test-marker")
            .then_some(text)
    });
    assert!(screen.contains("terminal-test-marker"));

    let document = sandbox.json_ok("default", &["terminal", "screen", &id]);
    assert_eq!(document["terminal"]["id"], id);
    assert!(
        document["lines"]
            .as_array()
            .is_some_and(|lines| lines.iter().any(|line| line == "terminal-test-marker")),
        "the JSON screen holds the marker line: {document}"
    );
}

#[test]
fn a_resize_changes_the_size_the_terminal_reports() {
    let sandbox = Sandbox::new();
    folder_workspace(&sandbox);
    let id = create_terminal(&sandbox)["id"]
        .as_str()
        .expect("terminal id")
        .to_owned();

    sandbox
        .run(
            "default",
            &["terminal", "resize", &id, "--cols", "100", "--rows", "30"],
        )
        .success();
    let shown = show(&sandbox, &id);
    assert_eq!(shown["cols"], 100);
    assert_eq!(shown["rows"], 30);
}

#[test]
fn a_killed_terminal_stays_listed_as_exited_and_closing_it_forgets_it() {
    let sandbox = Sandbox::new();
    folder_workspace(&sandbox);
    let id = create_terminal(&sandbox)["id"]
        .as_str()
        .expect("terminal id")
        .to_owned();

    sandbox.json_ok("default", &["terminal", "kill", &id, "--force"]);
    let exited = eventually("the killed terminal to exit", || {
        let shown = show(&sandbox, &id);
        (shown["state"] == "exited").then_some(shown)
    });
    assert_eq!(exited["exit"]["reason"], "killed");

    sandbox.json_ok("default", &["terminal", "close", &id]);
    sandbox
        .run("default", &["terminal", "show", &id])
        .code(1)
        .stderr(contains("terminal.not_found"));
}

#[test]
fn a_terminal_that_does_not_exist_is_reported_with_exit_code_1() {
    let sandbox = Sandbox::new();
    folder_workspace(&sandbox);

    let refused = sandbox
        .run(
            "default",
            &with_json(&["terminal", "send", "no-such-terminal", "x"]),
        )
        .code(1);
    assert_eq!(
        single_document(&refused)["error"]["code"],
        "terminal.not_found"
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
