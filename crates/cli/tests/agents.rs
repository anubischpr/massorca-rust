//! Agent sessions through the CLI, with the fake agent declared as kind `fake` in the test profile (ADR 0057).
//!
//! Every CLI call is its own process, so a session that is still listed as live has outlived the call that started it.

mod harness;

use harness::{Sandbox, eventually, single_document, stdout_text, with_json};
use predicates::str::contains;
use serde_json::{Value, json};

/// Declares kind `fake`: the fake agent binary, reporting through `massorca agent hook` (`env-command`).
fn declare_fake(sandbox: &Sandbox) {
    let kind = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
        "resume": ["--resume", "{ref}"],
        "status": {
            "mechanism": "env-command",
            "events": { "start": "idle", "prompt": "working", "stop": "done", "fail": "done:failure" },
            "result_field": "result"
        },
        "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] }
    });
    write_kind(sandbox, "fake.json", &kind);
}

fn write_kind(sandbox: &Sandbox, file: &str, kind: &Value) {
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join(file), kind.to_string()).expect("write a kind file");
}

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

/// Runs `--json <args>`, which must fail, and returns the error code.
fn error_code(sandbox: &Sandbox, args: &[&str]) -> String {
    let outcome = sandbox.run("default", &with_json(args)).failure();
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"]["code"]
        .as_str()
        .expect("an error code")
        .to_owned()
}

fn text<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("no {field} in {value}"))
}

/// Waits until the terminal's screen shows `line`.
fn wait_for_line(sandbox: &Sandbox, terminal: &str, line: &str) {
    eventually(&format!("`{line}` on the agent's screen"), || {
        let screen = sandbox.json_ok("default", &["terminal", "screen", terminal]);
        screen["lines"]
            .as_array()
            .is_some_and(|lines| {
                lines
                    .iter()
                    .any(|shown| shown.as_str().map(str::trim) == Some(line))
            })
            .then_some(())
    });
}

#[test]
fn a_started_agent_runs_in_its_workspace_and_stops_and_is_forgotten() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let workspace_id = folder_workspace(&sandbox);

    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            "main",
            "--prompt",
            "hello agent",
            "--title",
            "first",
        ],
    );
    let session = text(&started, "id").to_owned();
    let terminal = text(&started, "terminal_id").to_owned();
    assert_eq!(started["kind"], "fake");
    assert_eq!(started["workspace_id"], workspace_id);
    assert_eq!(started["title"], "first");
    assert_eq!(started["read_only"], false);
    assert!(
        started["agent_ref"].is_string(),
        "the ref is assigned at launch: {started}"
    );
    assert_eq!(started["execution"]["state"], "live");

    wait_for_line(&sandbox, &terminal, "fake-agent: done hello agent");

    // The start command has exited; the session is still live in later, separate calls.
    let listed = sandbox.json_ok("default", &["agent", "list", "--workspace", "main"]);
    assert_eq!(listed["sessions"][0]["id"], session.as_str());
    assert_eq!(listed["sessions"][0]["execution"]["state"], "live");
    let shown = sandbox.json_ok("default", &["agent", "status", &session[..18]]);
    assert_eq!(
        shown["id"],
        session.as_str(),
        "a long enough prefix selects the session"
    );
    assert_eq!(shown["execution"]["state"], "live");
    sandbox
        .run("default", &["agent", "status", &session])
        .success()
        .stdout(contains(format!("id: {session}")))
        .stdout(contains("execution: live"))
        .stdout(contains("activity: done (success) from hook"));

    assert_eq!(
        error_code(&sandbox, &["agent", "start", "fake", "--workspace", "main"]),
        "agent.workspace_busy"
    );
    let reader = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            "main",
            "--read-only",
        ],
    );
    assert_eq!(reader["read_only"], true);
    assert_eq!(reader["execution"]["state"], "live");

    assert_eq!(
        error_code(&sandbox, &["agent", "forget", &session]),
        "agent.busy"
    );
    let stopped = sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);
    assert_eq!(stopped["execution"]["state"], "exited");
    assert_eq!(stopped["execution"]["exit"]["reason"], "killed");
    let again = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(again["execution"]["exit"]["reason"], "killed");

    let forgotten = sandbox.json_ok("default", &["agent", "forget", &session]);
    assert_eq!(forgotten["id"], session.as_str());
    assert_eq!(
        error_code(&sandbox, &["agent", "status", &session]),
        "agent.not_found"
    );
    let remaining = sandbox.json_ok("default", &["agent", "list"]);
    let ids: Vec<&str> = remaining["sessions"]
        .as_array()
        .expect("sessions")
        .iter()
        .filter_map(|listed| listed["id"].as_str())
        .collect();
    assert_eq!(ids, [text(&reader, "id")]);

    // With the writer gone, the workspace takes a new writing session.
    let next = sandbox.json_ok(
        "default",
        &["agent", "start", "fake", "--workspace", "main"],
    );
    assert_eq!(next["execution"]["state"], "live");
}

#[test]
fn kinds_that_cannot_start_are_refused() {
    let sandbox = Sandbox::new();
    folder_workspace(&sandbox);
    let missing = sandbox.home().join("no-such-agent-program");
    write_kind(
        &sandbox,
        "ghost.json",
        &json!({ "id": "ghost", "name": "Ghost", "program": [missing.to_string_lossy()] }),
    );

    assert_eq!(
        error_code(
            &sandbox,
            &["agent", "start", "ghost", "--workspace", "main"]
        ),
        "agent.not_installed"
    );
    assert_eq!(
        error_code(
            &sandbox,
            &["agent", "start", "nobody", "--workspace", "main"]
        ),
        "agent.kind_not_found"
    );
    let empty = sandbox.json_ok("default", &["agent", "list"]);
    assert_eq!(empty["sessions"], json!([]));
    let listing = stdout_text(&sandbox.run("default", &["agent", "list"]).success());
    assert!(listing.contains("no agent sessions"), "{listing}");
}

#[test]
fn a_service_restart_leaves_the_session_exited_by_the_service_stopping() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    folder_workspace(&sandbox);
    let started = sandbox.json_ok(
        "default",
        &["agent", "start", "fake", "--workspace", "main"],
    );
    let session = text(&started, "id").to_owned();
    wait_for_line(&sandbox, text(&started, "terminal_id"), "fake-agent: ready");

    sandbox.run("default", &["service", "restart"]).success();

    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(shown["execution"]["state"], "exited");
    assert_eq!(shown["execution"]["exit"]["reason"], "service_stopped");
    sandbox
        .run("default", &["agent", "list"])
        .success()
        .stdout(contains("exited (service stopped)"));
}
