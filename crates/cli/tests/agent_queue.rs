//! The prompt queue through the CLI (decision 0084), with the fake agent. The fake agent reports `working` while it holds a
//! prompt that says `hold` until a line `release` arrives, and `waiting` while a prompt that says `ask:` waits for an
//! answer. A prompt it reads is echoed on the screen as `fake-agent: working on TEXT`.

mod harness;

use std::fs;

use harness::{Sandbox, eventually, single_document, with_json};
use serde_json::{Value, json};

/// Generous upper bound for `--until`; the wait ends as soon as the state is observed.
const UNTIL_SECS: &str = "60";

fn status() -> Value {
    json!({
        "mechanism": "env-command",
        "events": { "start": "idle", "prompt": "working", "stop": "done", "ask": "waiting" },
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
    fs::create_dir_all(&dir).expect("create the kinds directory");
    fs::write(dir.join(format!("{id}.json")), kind.to_string()).expect("write a kind file");
}

/// A sandbox with a folder workspace `main` and two kinds: `fake`, which reports its activity, and `fake-silent`, which
/// reports none and so has no queue.
fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    declare(&sandbox, "fake", status());
    declare(&sandbox, "fake-silent", json!({ "mechanism": "none" }));
    let notes = sandbox.home().join("notes");
    fs::create_dir_all(&notes).expect("create folder");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    sandbox
}

/// Starts `kind` read-only in `main` with a first prompt, and returns the session record.
fn start_with_prompt(sandbox: &Sandbox, kind: &str, prompt: &str) -> Value {
    sandbox.json_ok(
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
    )
}

/// Starts the fake agent holding a prompt, and waits until it reports `working`: a queue call made before that meets an
/// agent the service still counts as idle from its start.
fn start_busy(sandbox: &Sandbox) -> Value {
    let started = start_with_prompt(sandbox, "fake", "hold");
    until(sandbox, &id_of(&started), "working");
    started
}

fn id_of(record: &Value) -> String {
    record["id"].as_str().expect("id").to_owned()
}

fn terminal_of(record: &Value) -> String {
    record["terminal_id"]
        .as_str()
        .expect("terminal id")
        .to_owned()
}

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

/// Runs `--json <args>`, which must fail, and returns the error object.
fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure();
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

/// The agent's screen, trimmed lines.
fn screen(sandbox: &Sandbox, terminal: &str) -> Vec<String> {
    let screen = sandbox.json_ok("default", &["terminal", "screen", terminal]);
    screen["lines"]
        .as_array()
        .map(|lines| {
            lines
                .iter()
                .filter_map(|line| line.as_str().map(|line| line.trim().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

/// Waits until the terminal's screen shows `line`; returns the screen then.
fn wait_for_line(sandbox: &Sandbox, terminal: &str, line: &str) -> Vec<String> {
    eventually(&format!("`{line}` on the agent's screen"), || {
        let shown = screen(sandbox, terminal);
        shown.iter().any(|shown| shown == line).then_some(shown)
    })
}

/// The position of `line` on a screen, which must show it.
fn line_index(shown: &[String], line: &str) -> usize {
    shown
        .iter()
        .position(|shown| shown == line)
        .unwrap_or_else(|| panic!("`{line}` not on the screen: {shown:?}"))
}

fn queue_add(sandbox: &Sandbox, session: &str, text: &str) -> Value {
    sandbox.json_ok("default", &["agent", "queue", "add", session, text])
}

fn queue_list(sandbox: &Sandbox, session: &str) -> Value {
    sandbox.json_ok("default", &["agent", "queue", "list", session])
}

fn stop(sandbox: &Sandbox, session: &str) {
    sandbox.json_ok("default", &["agent", "stop", session, "--force"]);
}

#[test]
fn a_prompt_queued_behind_a_busy_agent_is_listed_edited_and_removed() {
    let sandbox = sandbox();
    let session = id_of(&start_busy(&sandbox));

    let added = queue_add(&sandbox, &session, "later");
    let id = added["id"].as_str().expect("queued prompt id").to_owned();
    assert_eq!(added["position"], 1, "{added}");
    assert_eq!(added["state"], "waiting", "{added}");
    assert_eq!(added["reason"], "busy", "{added}");
    assert_eq!(added["text"], "later", "{added}");

    let listed = queue_list(&sandbox, &session);
    assert_eq!(listed["session"], session.as_str(), "{listed}");
    assert_eq!(listed["items"][0]["id"], id.as_str(), "{listed}");

    let edited = sandbox.json_ok("default", &["agent", "queue", "edit", &id, "edited"]);
    assert_eq!(edited["text"], "edited", "{edited}");
    assert_eq!(edited["state"], "waiting", "{edited}");

    let removed = sandbox.json_ok("default", &["agent", "queue", "remove", &id]);
    assert_eq!(removed["id"], id.as_str(), "{removed}");
    assert_eq!(queue_list(&sandbox, &session)["items"], json!([]));

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
    let session = id_of(&started);

    let error = error_of(&sandbox, &["agent", "queue", "add", &session, "later"]);
    assert_eq!(error["code"], "agent.queue_unavailable", "{error}");

    stop(&sandbox, &session);
}

#[test]
fn queued_prompts_are_typed_in_order_once_the_agent_is_done() {
    let sandbox = sandbox();
    let started = start_busy(&sandbox);
    let session = id_of(&started);
    let terminal = terminal_of(&started);
    let one = queue_add(&sandbox, &session, "one");
    let two = queue_add(&sandbox, &session, "two");
    assert_eq!(
        (one["position"].clone(), two["position"].clone()),
        (json!(1), json!(2))
    );
    let listed = queue_list(&sandbox, &session);
    assert_eq!(listed["items"][0]["reason"], "busy", "{listed}");
    assert_eq!(listed["items"][1]["reason"], "busy", "{listed}");

    sandbox.json_ok(
        "default",
        &["agent", "prompt", &session, "release", "--force"],
    );
    let shown = wait_for_line(&sandbox, &terminal, "fake-agent: done two");
    let first = line_index(&shown, "fake-agent: working on one");
    let second = line_index(&shown, "fake-agent: working on two");
    assert!(first < second, "delivered in order: {shown:?}");
    assert_eq!(queue_list(&sandbox, &session)["items"], json!([]));

    stop(&sandbox, &session);
}

#[test]
fn a_queued_prompt_waits_while_the_agent_asks_and_send_now_is_refused() {
    let sandbox = sandbox();
    let started = start_with_prompt(&sandbox, "fake", "ask: go on");
    let session = id_of(&started);
    let terminal = terminal_of(&started);
    until(&sandbox, &session, "waiting");

    let queued = queue_add(&sandbox, &session, "later");
    assert_eq!(queued["reason"], "asking", "{queued}");
    let id = id_of(&queued);
    let error = error_of(&sandbox, &["agent", "queue", "send-now", &id]);
    assert_eq!(error["code"], "agent.busy", "{error}");
    let still = queue_list(&sandbox, &session);
    assert_eq!(still["items"][0]["state"], "waiting", "{still}");
    assert_eq!(still["items"][0]["reason"], "asking", "{still}");

    sandbox.json_ok("default", &["agent", "prompt", &session, "yes", "--force"]);
    wait_for_line(&sandbox, &terminal, "fake-agent: done later");
    assert_eq!(queue_list(&sandbox, &session)["items"], json!([]));

    stop(&sandbox, &session);
}

#[test]
fn send_now_types_a_queued_prompt_at_once_and_removes_it() {
    let sandbox = sandbox();
    let started = start_busy(&sandbox);
    let session = id_of(&started);
    let terminal = terminal_of(&started);
    let queued = queue_add(&sandbox, &session, "release");
    let id = id_of(&queued);

    let sent = sandbox.json_ok("default", &["agent", "queue", "send-now", &id]);
    assert_eq!(sent["id"], id.as_str(), "{sent}");
    wait_for_line(&sandbox, &terminal, "fake-agent: released");
    assert_eq!(queue_list(&sandbox, &session)["items"], json!([]));

    stop(&sandbox, &session);
}

#[test]
fn a_stop_pauses_the_queue_until_the_queue_is_resumed() {
    let sandbox = sandbox();
    let session = id_of(&start_busy(&sandbox));
    queue_add(&sandbox, &session, "one");
    stop(&sandbox, &session);
    assert_eq!(
        queue_list(&sandbox, &session)["items"][0]["reason"],
        "paused_stopped"
    );

    let resumed = sandbox.json_ok("default", &["agent", "resume", &session]);
    until(&sandbox, &session, "idle");
    let held = queue_list(&sandbox, &session);
    assert_eq!(held["items"][0]["reason"], "paused_stopped", "{held}");

    sandbox.json_ok("default", &["agent", "queue", "resume", &session]);
    wait_for_line(&sandbox, &terminal_of(&resumed), "fake-agent: done one");
    assert_eq!(queue_list(&sandbox, &session)["items"], json!([]));

    stop(&sandbox, &session);
}

#[test]
fn a_service_restart_pauses_the_queue_as_restarted() {
    let sandbox = sandbox();
    let session = id_of(&start_busy(&sandbox));
    queue_add(&sandbox, &session, "one");

    sandbox.run("default", &["service", "stop"]).success();
    sandbox.wait_for_running("default", false);
    let after = queue_list(&sandbox, &session);
    assert_eq!(after["items"][0]["state"], "waiting", "{after}");
    assert_eq!(after["items"][0]["reason"], "paused_restarted", "{after}");
}

#[test]
fn a_bring_back_prompt_does_not_lift_a_pause_the_person_set() {
    let sandbox = sandbox();
    let session = id_of(&start_busy(&sandbox));
    queue_add(&sandbox, &session, "one");
    stop(&sandbox, &session);
    assert_eq!(
        queue_list(&sandbox, &session)["items"][0]["reason"],
        "paused_stopped"
    );
    sandbox.json_ok("default", &["agent", "resume", &session]);
    until(&sandbox, &session, "idle");

    let child = sandbox.json_ok(
        "default",
        &[
            "agent",
            "delegate",
            "--from",
            &session,
            "--prompt",
            "look",
            "--read-only",
        ],
    );
    let child_id = id_of(&child);
    until(&sandbox, &child_id, "done");
    sandbox.json_ok("default", &["agent", "bring-back", &child_id, "--no-merge"]);

    let held = queue_list(&sandbox, &session);
    assert_eq!(held["items"][0]["reason"], "paused_stopped", "{held}");
    stop(&sandbox, &session);
}
