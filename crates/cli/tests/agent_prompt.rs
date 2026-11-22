//! Typed prompts through the CLI (decision 0054). The fake agent is declared as kinds in the test profile; the tests
//! wait on observed states (`agent status --until`), never on sleeps.

mod harness;

use harness::{Sandbox, single_document, with_json};
use predicates::str::contains;
use serde_json::{Value, json};

/// Generous upper bound for `--until`; the wait ends as soon as the state is observed.
const UNTIL_SECS: &str = "60";
/// Most bytes of a prompt (0054).
const MAX_PROMPT_BYTES: usize = 8 * 1024;

/// The status table every fake kind declares: start is ready, a prompt works, a `done` carries the turn's result.
fn status() -> Value {
    json!({
        "mechanism": "env-command",
        "events": {
            "start": "idle", "prompt": "working", "ask": "waiting",
            "stop": "done", "fail": "done:failure"
        },
        "result_field": "result"
    })
}

/// Declares kind `id`, which runs the fake agent, with an argv prompt and the given status.
fn declare(sandbox: &Sandbox, id: &str, status: Value) {
    let kind = json!({
        "id": id,
        "name": format!("Fake agent {id}"),
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
        "resume": ["--resume", "{ref}"],
        "status": status
    });
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join(format!("{id}.json")), kind.to_string()).expect("write a kind file");
}

/// A sandbox with kind `fake` and a folder workspace `main`.
fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    declare(&sandbox, "fake", status());
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    sandbox
}

/// `agent start <kind> --workspace main <extra>`, which must succeed; returns the session record.
fn start(sandbox: &Sandbox, kind: &str, extra: &[&str]) -> Value {
    let mut args = vec!["agent", "start", kind, "--workspace", "main"];
    args.extend_from_slice(extra);
    sandbox.json_ok("default", &args)
}

fn session_of(record: &Value) -> String {
    record["id"].as_str().expect("session id").to_owned()
}

fn terminal_of(record: &Value) -> String {
    record["terminal_id"]
        .as_str()
        .expect("terminal id")
        .to_owned()
}

/// `agent status <session> --until <state>`, which must succeed; returns the record it printed.
fn until(sandbox: &Sandbox, session: &str, state: &str) -> Value {
    sandbox.json_ok(
        "default",
        &[
            "agent",
            "status",
            session,
            "--until",
            state,
            "--timeout",
            UNTIL_SECS,
        ],
    )
}

fn activity(session: &Value) -> (&str, Option<&str>, &str) {
    let activity = &session["activity"];
    (
        activity["state"].as_str().unwrap_or_default(),
        activity["outcome"].as_str(),
        activity["source"].as_str().unwrap_or_default(),
    )
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

#[test]
fn a_prompt_to_an_idle_agent_drives_it_to_working_then_done() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake", &[]);
    let session = session_of(&started);
    let terminal = terminal_of(&started);
    until(&sandbox, &session, "idle");

    sandbox
        .run("default", &["agent", "prompt", &session, "hold this"])
        .success()
        .stdout(contains("handed to the terminal: 10 bytes"));
    let working = until(&sandbox, &session, "working");
    assert_eq!(activity(&working), ("working", None, "hook"));

    sandbox
        .run("default", &["terminal", "send", &terminal, "release"])
        .success();
    let done = until(&sandbox, &session, "done");
    assert_eq!(activity(&done), ("done", Some("success"), "hook"));
    assert_eq!(done["result"], "done: hold this");
}

#[test]
fn a_prompt_while_the_agent_works_is_refused_as_busy_unless_forced() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake", &["--prompt", "hold busy"]);
    let session = session_of(&started);
    let terminal = terminal_of(&started);
    until(&sandbox, &session, "working");

    assert_eq!(
        error_code(&sandbox, &["agent", "prompt", &session, "second"]),
        "agent.busy"
    );
    let forced = sandbox.json_ok(
        "default",
        &["agent", "prompt", &session, "second", "--force"],
    );
    assert_eq!(forced["receipt"], "handed_to_terminal");
    assert_eq!(forced["bytes"], 7, "the text and its carriage return");

    sandbox
        .run("default", &["terminal", "send", &terminal, "release"])
        .success();
    let done = until(&sandbox, &session, "done");
    assert_eq!(done["result"], "done: hold busy");
}

#[test]
fn a_prompt_to_an_exited_run_is_refused_as_not_live() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake", &[]);
    let session = session_of(&started);
    until(&sandbox, &session, "idle");
    sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);

    assert_eq!(
        error_code(&sandbox, &["agent", "prompt", &session, "too late"]),
        "agent.not_live"
    );
}

#[test]
fn a_prompt_over_the_size_limit_is_refused_before_anything_is_typed() {
    let sandbox = sandbox();
    let started = start(&sandbox, "fake", &[]);
    let session = session_of(&started);
    until(&sandbox, &session, "idle");

    let long = "a".repeat(MAX_PROMPT_BYTES + 1);
    assert_eq!(
        error_code(&sandbox, &["agent", "prompt", &session, &long]),
        "agent.prompt_invalid"
    );
    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(activity(&shown), ("idle", None, "hook"), "still idle");
}
