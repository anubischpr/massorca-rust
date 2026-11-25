//! A delegated child reports back to its parent, and a parent waits for a writing child that shares its workspace
//! (decision 0170). A git project with a parent workspace `lead` runs the fake agent; the tests wait on observed states
//! (`agent status --until`) or on the agent's screen, never on sleeps.

mod harness;

use std::path::PathBuf;

use harness::{Sandbox, eventually, git_repo, single_document, with_json};
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
        "resume": ["--resume", "{ref}"],
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

fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure();
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
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

#[test]
fn a_parent_waits_while_its_writing_child_shares_its_worktree() {
    let (sandbox, lead) = git_sandbox();
    let (parent, terminal) = start_parent(&sandbox);
    let delegated = delegate(
        &sandbox,
        &parent,
        "hold then write:shared.txt",
        &["--same-workspace", "--title", "Sharer"],
    );
    let child = id_of(&delegated);
    let parent_record = sandbox.json_ok("default", &["agent", "status", &parent]);
    assert_eq!(
        delegated["workspace_id"], parent_record["workspace_id"],
        "{delegated}"
    );
    assert_eq!(
        delegated["note"],
        "The child works in its parent's worktree, and the parent waits until it finishes."
    );
    until(&sandbox, &child, "working");

    let queued = sandbox.json_ok("default", &["agent", "queue", "add", &parent, "next step"]);
    assert_eq!(queued["reason"], "child_writing", "{queued}");
    let refused = error_of(&sandbox, &["agent", "prompt", &parent, "now"]);
    assert_eq!(refused["code"], "agent.busy", "{refused}");
    assert_eq!(refused["data"]["child"], child.as_str(), "{refused}");
    assert!(
        !screen(&sandbox, &terminal)
            .iter()
            .any(|line| line.contains("next step")),
        "the parent waits"
    );

    sandbox.json_ok(
        "default",
        &["agent", "prompt", &child, "release", "--force"],
    );
    until(&sandbox, &child, "done");
    assert!(
        lead.join("shared.txt").is_file(),
        "the child wrote in the parent's worktree"
    );
    // Both what waited and the child's notice reach the parent once the child is done, in queue order.
    let shown = wait_for_text(&sandbox, &terminal, &notice_line(&child));
    let first = shown
        .iter()
        .position(|line| line.contains("working on next step"))
        .expect("the waiting prompt was typed");
    let notice = shown
        .iter()
        .position(|line| line.contains(&notice_line(&child)))
        .expect("the notice was typed");
    assert!(first < notice, "{shown:?}");
    stop(&sandbox, &parent);
}

#[test]
fn a_second_writing_child_in_the_parents_worktree_is_refused_while_the_first_works() {
    let (sandbox, _lead) = git_sandbox();
    let (parent, _terminal) = start_parent(&sandbox);
    let first = id_of(&delegate(
        &sandbox,
        &parent,
        "hold on",
        &["--same-workspace", "--title", "First"],
    ));
    until(&sandbox, &first, "working");

    let refused = error_of(
        &sandbox,
        &[
            "agent",
            "delegate",
            "--from",
            &parent,
            "--prompt",
            "look",
            "--same-workspace",
        ],
    );
    assert_eq!(refused["code"], "agent.workspace_busy", "{refused}");
    assert_eq!(refused["data"]["session"], first.as_str(), "{refused}");
    stop(&sandbox, &parent);
}

#[test]
fn a_child_is_not_resumed_beside_a_sibling_writing_in_the_parents_worktree() {
    let (sandbox, _lead) = git_sandbox();
    let (parent, _terminal) = start_parent(&sandbox);
    let first = id_of(&delegate(
        &sandbox,
        &parent,
        "look",
        &["--same-workspace", "--title", "First"],
    ));
    until(&sandbox, &first, "done");
    stop(&sandbox, &first);
    let second = id_of(&delegate(
        &sandbox,
        &parent,
        "hold on",
        &["--same-workspace", "--title", "Second"],
    ));
    until(&sandbox, &second, "working");

    let refused = error_of(&sandbox, &["agent", "resume", &first]);
    assert_eq!(refused["code"], "agent.workspace_busy", "{refused}");
    assert_eq!(refused["data"]["session"], second.as_str(), "{refused}");
    stop(&sandbox, &parent);
}

#[test]
fn a_grandchild_is_not_picked_into_a_worktree_where_its_root_runs() {
    let (sandbox, _lead) = git_sandbox();
    sandbox.json_ok(
        "default",
        &["config", "set", "agents.delegation_depth", "2"],
    );
    let (parent, _terminal) = start_parent(&sandbox);
    let child = id_of(&delegate(
        &sandbox,
        &parent,
        "look",
        &["--same-workspace", "--title", "Child"],
    ));
    until(&sandbox, &child, "done");

    // The root's turn is over, but only the child would wait for the grandchild: the root would write beside it.
    let refused = error_of(
        &sandbox,
        &[
            "agent",
            "delegate",
            "--from",
            &child,
            "--prompt",
            "look",
            "--same-workspace",
        ],
    );
    assert_eq!(refused["code"], "agent.workspace_busy", "{refused}");
    assert_eq!(refused["data"]["session"], parent.as_str(), "{refused}");
    stop(&sandbox, &parent);
}

#[test]
fn same_workspace_takes_no_workspace_name() {
    let (sandbox, _lead) = git_sandbox();
    let (parent, _terminal) = start_parent(&sandbox);
    let outcome = sandbox
        .run(
            "default",
            &[
                "agent",
                "delegate",
                "--from",
                &parent,
                "--prompt",
                "x",
                "--same-workspace",
                "--name",
                "other",
            ],
        )
        .failure();
    let text = String::from_utf8_lossy(&outcome.get_output().stderr).into_owned();
    assert!(text.contains("--same-workspace"), "{text}");
    stop(&sandbox, &parent);
}
