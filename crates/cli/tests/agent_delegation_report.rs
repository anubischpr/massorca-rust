//! A delegated child reports back to its parent (decision 0170). A git project with a parent workspace `lead` runs the fake agent; the tests wait on observed states
//! (`agent status --until`) or on the agent's screen, never on sleeps.

mod harness;

use std::path::PathBuf;

use harness::{Sandbox, eventually, git_repo};
use serde_json::{Value, json};

/// Generous upper bound for `--until`; the wait ends as soon as the state is observed.
const UNTIL_SECS: &str = "60";

/// Declares kind `fake`, which runs the fake agent with an argv prompt and reports its activity.
fn declare_fake(sandbox: &Sandbox) {
    let kind = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
        "status": {
            "mechanism": "env-command",
            "events": {
                "start": "idle", "prompt": "working", "ask": "waiting",
                "stop": "done", "fail": "done:failure"
            },
            "result_field": "result"
        }
    });
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join("fake.json"), kind.to_string()).expect("write a kind file");
}

/// A sandbox with kind `fake`, a git project `repo` and its worktree workspace `lead`; returns the sandbox and the path
/// of `lead`.
fn git_sandbox() -> (Sandbox, PathBuf) {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    let repo_text = repo.to_string_lossy().into_owned();
    let root_text = sandbox.home().join("ws").to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &[
            "project",
            "add",
            repo_text.as_str(),
            "--workspaces-root",
            root_text.as_str(),
        ],
    );
    let lead = sandbox.json_ok("default", &["workspace", "create", "repo", "lead"]);
    let path = PathBuf::from(lead["path"].as_str().expect("workspace path"));
    (sandbox, path)
}

/// Starts the fake agent in workspace `lead`, waits for it to be idle, and returns its session id and terminal.
fn start_parent(sandbox: &Sandbox) -> (String, String) {
    let started = sandbox.json_ok(
        "default",
        &["agent", "start", "fake", "--workspace", "lead"],
    );
    let session = id_of(&started);
    let terminal = started["terminal_id"]
        .as_str()
        .expect("terminal id")
        .to_owned();
    until(sandbox, &session, "idle");
    (session, terminal)
}

fn delegate(sandbox: &Sandbox, parent: &str, prompt: &str, extra: &[&str]) -> Value {
    let mut args = vec!["agent", "delegate", "--from", parent, "--prompt", prompt];
    args.extend_from_slice(extra);
    sandbox.json_ok("default", &args)
}

fn id_of(record: &Value) -> String {
    record["id"].as_str().expect("id").to_owned()
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

/// The terminal's screen lines.
fn screen(sandbox: &Sandbox, terminal: &str) -> Vec<String> {
    let shown = sandbox.json_ok("default", &["terminal", "screen", terminal]);
    shown["lines"]
        .as_array()
        .expect("lines")
        .iter()
        .filter_map(|line| line.as_str().map(str::to_owned))
        .collect()
}

/// Waits until a line of the terminal's screen contains `text`; returns the screen then.
fn wait_for_text(sandbox: &Sandbox, terminal: &str, text: &str) -> Vec<String> {
    eventually(&format!("`{text}` on the agent's screen"), || {
        let lines = screen(sandbox, terminal);
        lines
            .iter()
            .any(|line| line.contains(text))
            .then_some(lines)
    })
}

/// The text of the notice line the parent's fake agent echoes when it starts working on the notice of `child`.
fn notice_line(child: &str) -> String {
    format!("working on MassOrca: child session {child}")
}

fn queue_list(sandbox: &Sandbox, session: &str) -> Value {
    sandbox.json_ok("default", &["agent", "queue", "list", session])
}

fn stop(sandbox: &Sandbox, session: &str) {
    sandbox.json_ok("default", &["agent", "stop", session, "--force"]);
}

#[test]
fn a_finished_child_is_told_to_its_parent_with_its_outcome_and_without_its_result() {
    let (sandbox, _lead) = git_sandbox();
    let (parent, terminal) = start_parent(&sandbox);
    let child = id_of(&delegate(
        &sandbox,
        &parent,
        "write:done.txt",
        &["--title", "Writer"],
    ));

    let shown = wait_for_text(&sandbox, &terminal, &notice_line(&child));
    let text = shown.join("");
    assert!(text.contains("finished: done."), "{shown:?}");
    assert!(
        !text.contains("Writer"),
        "the title is not typed: {shown:?}"
    );
    assert!(
        !text.contains("done: write:done.txt"),
        "the child's result is not quoted: {shown:?}"
    );
    // The parent read the notice as one prompt and answered it; nothing is left queued.
    until(&sandbox, &parent, "done");
    assert_eq!(queue_list(&sandbox, &parent)["items"], json!([]));
    stop(&sandbox, &parent);
}

#[test]
fn a_failed_and_a_stopped_child_are_told_as_such() {
    let (sandbox, _lead) = git_sandbox();
    let (parent, terminal) = start_parent(&sandbox);
    let failed = id_of(&delegate(
        &sandbox,
        &parent,
        "fail here",
        &["--read-only", "--title", "Reader"],
    ));
    wait_for_text(&sandbox, &terminal, &notice_line(&failed));
    let shown = wait_for_text(&sandbox, &terminal, "finished: failed.");
    assert!(shown.join("").contains(&failed), "{shown:?}");

    let held = id_of(&delegate(
        &sandbox,
        &parent,
        "hold on",
        &["--read-only", "--title", "Waiter"],
    ));
    until(&sandbox, &held, "working");
    stop(&sandbox, &held);
    let shown = wait_for_text(&sandbox, &terminal, &notice_line(&held));
    let text = shown.join("");
    assert!(text.contains("finished: stopped."), "{shown:?}");
    stop(&sandbox, &parent);
}
