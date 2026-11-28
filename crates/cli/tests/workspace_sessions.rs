//! `massorca workspace sessions <WS> [--stop]`: a workspace's agent sessions and terminals, and stopping every live
//! run and terminal in it while the workspace itself stays. The fake agent runs in a git project's worktree.

mod harness;

use std::path::PathBuf;

use harness::{Sandbox, git_repo, stdout_text};
use predicates::str::contains;
use serde_json::{Value, json};

/// Generous upper bound for `--until`; the wait ends as soon as the state is observed.
const UNTIL_SECS: &str = "60";

/// Declares kind `fake`, which runs the fake agent and reports its activity through `massorca agent hook`.
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
        }
    });
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join("fake.json"), kind.to_string()).expect("write a kind file");
}

/// A sandbox with kind `fake` and a git project `repo` whose workspaces go under `<home>/ws`.
fn git_sandbox() -> Sandbox {
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
    sandbox
}

fn text<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("no {field} in {value}"))
}

/// The `id`s of the array under `field`.
fn ids(value: &Value, field: &str) -> Vec<String> {
    value[field]
        .as_array()
        .unwrap_or_else(|| panic!("no {field} array in {value}"))
        .iter()
        .map(|item| text(item, "id").to_owned())
        .collect()
}

/// The string ids of the array under `field`, such as the `terminals` a stop answer lists.
fn ids_of_strings(value: &Value, field: &str) -> Vec<String> {
    value[field]
        .as_array()
        .unwrap_or_else(|| panic!("no {field} array in {value}"))
        .iter()
        .map(|item| item.as_str().expect("an id").to_owned())
        .collect()
}

fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}

/// Starts the fake agent in `workspace` and waits for it to be idle; returns its session id.
fn start(sandbox: &Sandbox, workspace: &str) -> String {
    let started = sandbox.json_ok(
        "default",
        &["agent", "start", "fake", "--workspace", workspace],
    );
    let session = text(&started, "id").to_owned();
    sandbox.json_ok(
        "default",
        &[
            "agent",
            "status",
            &session,
            "--until",
            "idle",
            "--timeout",
            UNTIL_SECS,
        ],
    );
    session
}

#[test]
fn sessions_lists_the_agent_sessions_and_terminals_of_one_workspace() {
    let sandbox = git_sandbox();
    sandbox.json_ok("default", &["workspace", "create", "repo", "main"]);
    sandbox.json_ok("default", &["workspace", "create", "repo", "other"]);
    let session = start(&sandbox, "main");
    let terminal = sandbox.json_ok("default", &["terminal", "create", "main"]);
    let terminal_id = text(&terminal, "id").to_owned();

    let listed = sandbox.json_ok("default", &["workspace", "sessions", "main"]);
    assert_eq!(ids(&listed, "sessions"), vec![session.clone()], "{listed}");
    // The agent's own run terminal is a terminal of the workspace too, as `terminal list --workspace` shows it.
    let agent_terminal = text(&listed["sessions"][0], "terminal_id").to_owned();
    assert_eq!(
        sorted(ids(&listed, "terminals")),
        sorted(vec![agent_terminal, terminal_id]),
        "{listed}"
    );

    let other = sandbox.json_ok("default", &["workspace", "sessions", "other"]);
    assert!(ids(&other, "sessions").is_empty(), "{other}");
    assert!(ids(&other, "terminals").is_empty(), "{other}");

    let text_out = sandbox
        .run("default", &["workspace", "sessions", "main"])
        .success();
    assert!(
        stdout_text(&text_out).contains(&session),
        "{}",
        stdout_text(&text_out)
    );
}

#[test]
fn sessions_stop_ends_every_live_run_and_terminal_and_keeps_the_workspace() {
    let sandbox = git_sandbox();
    let created = sandbox.json_ok("default", &["workspace", "create", "repo", "main"]);
    let path = PathBuf::from(text(&created, "path"));
    let session = start(&sandbox, "main");
    let terminal = sandbox.json_ok("default", &["terminal", "create", "main"]);
    let terminal_id = text(&terminal, "id").to_owned();
    let before = sandbox.json_ok("default", &["workspace", "sessions", "main"]);
    let agent_terminal = text(&before["sessions"][0], "terminal_id").to_owned();

    let stopped = sandbox.json_ok("default", &["workspace", "sessions", "main", "--stop"]);
    assert_eq!(stopped["agents"], json!([session]), "{stopped}");
    assert_eq!(
        sorted(ids_of_strings(&stopped, "terminals")),
        sorted(vec![agent_terminal, terminal_id.clone()]),
        "{stopped}"
    );

    assert!(path.is_dir(), "the workspace folder stays");
    sandbox.json_ok("default", &["workspace", "show", "main"]);
    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(shown["execution"]["state"], "exited", "{shown}");
    let terminal_shown = sandbox.json_ok("default", &["terminal", "show", &terminal_id]);
    assert!(
        !terminal_shown["exit"].is_null(),
        "the terminal has exited: {terminal_shown}"
    );

    let again = sandbox.json_ok("default", &["workspace", "sessions", "main", "--stop"]);
    assert!(ids_of_strings(&again, "agents").is_empty(), "{again}");
    assert!(ids_of_strings(&again, "terminals").is_empty(), "{again}");
}

#[test]
fn sessions_stop_of_a_workspace_with_nothing_running_says_so_in_text() {
    let sandbox = git_sandbox();
    sandbox.json_ok("default", &["workspace", "create", "repo", "main"]);
    sandbox
        .run("default", &["workspace", "sessions", "main", "--stop"])
        .success()
        .stdout(contains("nothing was running"));
}
