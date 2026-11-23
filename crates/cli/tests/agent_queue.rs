//! The prompt queue through the CLI (decision 0084): add, list, edit and remove, with the fake agent. The fake agent
//! reports `working` while it holds a prompt that says `hold`, so a prompt queued meanwhile waits.

mod harness;

use harness::{Sandbox, with_json};
use serde_json::{Value, json};

fn status() -> Value {
    json!({
        "mechanism": "env-command",
        "events": { "start": "idle", "prompt": "working", "stop": "done" },
        "result_field": "result"
    })
}

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
    sandbox.prepare_profile("default");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join(format!("{id}.json")), kind.to_string()).expect("write a kind file");
}

/// A sandbox with a folder workspace `main` and two kinds: `fake`, which reports its activity, and `fake-silent`, which
/// reports none and so has no queue.
fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    declare(&sandbox, "fake", status());
    declare(&sandbox, "fake-silent", json!({ "mechanism": "none" }));
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

/// Starts `kind` read-only in `main` with a first prompt, so the run is busy until its prompt is released.
fn start_busy(sandbox: &Sandbox, kind: &str, prompt: &str) -> String {
    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            kind,
            "--workspace",
            "main",
            "--read-only",
            "--prompt",
            prompt,
        ],
    );
    started["id"].as_str().expect("session id").to_owned()
}

/// Runs `--json <args>`, which must fail, and returns the error object.
fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure();
    let document = harness::single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

fn stop(sandbox: &Sandbox, session: &str) {
    sandbox.json_ok("default", &["agent", "stop", session, "--force"]);
}

#[test]
fn a_prompt_queued_behind_a_busy_agent_is_listed_edited_and_removed() {
    let sandbox = sandbox();
    let session = start_busy(&sandbox, "fake", "hold");

    let added = sandbox.json_ok("default", &["agent", "queue", "add", &session, "later"]);
    let id = added["id"].as_str().expect("queued prompt id").to_owned();
    assert_eq!(added["position"], 1, "{added}");
    assert_eq!(added["state"], "waiting", "{added}");
    assert_eq!(added["reason"], "busy", "{added}");
    assert_eq!(added["text"], "later", "{added}");

    let listed = sandbox.json_ok("default", &["agent", "queue", "list", &session]);
    assert_eq!(listed["session"], session.as_str(), "{listed}");
    assert_eq!(listed["items"][0]["id"], id.as_str(), "{listed}");

    let edited = sandbox.json_ok("default", &["agent", "queue", "edit", &id, "edited"]);
    assert_eq!(edited["text"], "edited", "{edited}");
    assert_eq!(edited["state"], "waiting", "{edited}");

    let removed = sandbox.json_ok("default", &["agent", "queue", "remove", &id]);
    assert_eq!(removed["id"], id.as_str(), "{removed}");
    let after = sandbox.json_ok("default", &["agent", "queue", "list", &session]);
    assert_eq!(after["items"], json!([]), "{after}");

    stop(&sandbox, &session);
}

#[test]
fn a_kind_that_reports_no_activity_has_no_queue() {
    let sandbox = sandbox();
    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake-silent",
            "--workspace",
            "main",
            "--read-only",
        ],
    );
    let session = started["id"].as_str().expect("session id").to_owned();

    let error = error_of(&sandbox, &["agent", "queue", "add", &session, "later"]);
    assert_eq!(error["code"], "agent.queue_unavailable", "{error}");

    stop(&sandbox, &session);
}
