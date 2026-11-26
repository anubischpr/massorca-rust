//! What a run is given to delegate with, and where a delegated child writes (decision 0161): `massorca`'s folder first on
//! the run's `PATH`, the instructions through the kind's channel while the session may still delegate, a writing child
//! of a plain folder in that folder, and each session's `delegation` options. The service runs with a temporary user
//! home and no vendor variables; the agent is the fake one, which prints whether it got the text, never the text.

mod harness;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use harness::{ChildGuard, Sandbox, eventually, git, git_repo, massorca_in};
use serde_json::{Value, json};

/// Generous upper bound for `--until`; the wait ends as soon as the state is observed.
const UNTIL_SECS: &str = "60";
/// The note of a writing child of a plain folder, verbatim.
const FOLDER_NOTE: &str = "This workspace has no Git, so the child works in this folder.";

/// Declares kind `fake`: the fake agent, reporting through `massorca agent hook`, with an instruction channel.
fn declare_fake(sandbox: &Sandbox) {
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    fs::create_dir_all(&dir).expect("create the kinds directory");
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
        },
        "instructions": {
            "args": ["--instructions", "{text}"],
            "supported_when": { "args": ["--help"], "contains": "--instructions" }
        }
    });
    fs::write(dir.join("fake.json"), kind.to_string()).expect("write the fake kind");
}

/// Starts the service with a temporary user home and none of the vendors' variables, and the fake agent asked to say
/// whether `massorca`'s folder is first on its `PATH`; `extra` is set on the service too.
fn spawn_service(sandbox: &Sandbox, extra: &[(&str, &str)]) -> ChildGuard {
    sandbox.track("default");
    let user_home = sandbox.home().join("user");
    let known = [
        ("APPDATA", user_home.join("AppData").join("Roaming")),
        ("LOCALAPPDATA", user_home.join("AppData").join("Local")),
        ("XDG_CONFIG_HOME", user_home.join(".config")),
        ("XDG_DATA_HOME", user_home.join(".local").join("share")),
        ("XDG_STATE_HOME", user_home.join(".local").join("state")),
        ("XDG_CACHE_HOME", user_home.join(".cache")),
    ];
    let mut command = massorca_in(sandbox.home());
    command
        .args(["--profile", "default", "service", "run"])
        .env("HOME", &user_home)
        .env("USERPROFILE", &user_home)
        .env("FAKE_AGENT_PRINT_PATH", "1")
        .env_remove("FAKE_AGENT_HOME")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (name, folder) in &known {
        fs::create_dir_all(folder).expect("create a known folder");
        command.env(name, folder);
    }
    for name in [
        "CLAUDE_CONFIG_DIR",
        "CODEX_HOME",
        "PI_CODING_AGENT_DIR",
        "FAKE_AGENT_HELP_WITHOUT_INSTRUCTIONS",
        "FAKE_AGENT_HELP_MARKER",
    ] {
        command.env_remove(name);
    }
    for (name, value) in extra {
        command.env(name, value);
    }
    let guard = ChildGuard::new(command.spawn().expect("the service starts"));
    sandbox.wait_for_running("default", true);
    guard
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

/// A plain folder project `notes` with its workspace `main`.
fn folder_workspace(sandbox: &Sandbox) {
    let notes = sandbox.home().join("notes");
    fs::create_dir_all(&notes).expect("create the folder");
    sandbox.json_ok(
        "default",
        &["project", "add", path_text(&notes), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
}

/// A git project `repo` whose workspaces go under `<home>/ws`, with its worktree workspace `lead`.
fn worktree_workspace(sandbox: &Sandbox) -> PathBuf {
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    git(&repo, &["config", "user.name", "t"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    let root = sandbox.home().join("ws");
    sandbox.json_ok(
        "default",
        &[
            "project",
            "add",
            path_text(&repo),
            "--workspaces-root",
            path_text(&root),
        ],
    );
    let lead = sandbox.json_ok("default", &["workspace", "create", "repo", "lead"]);
    PathBuf::from(lead["path"].as_str().expect("workspace path"))
}

fn id_of(record: &Value) -> String {
    record["id"].as_str().expect("session id").to_owned()
}

fn terminal_of(record: &Value) -> String {
    record["terminal_id"]
        .as_str()
        .expect("terminal id")
        .to_owned()
}

/// `agent status <session> --until <state>`; returns the record it printed.
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

/// Starts the fake agent in `workspace` and waits until it is idle; returns its record then.
fn start_parent(sandbox: &Sandbox, workspace: &str) -> Value {
    let started = sandbox.json_ok(
        "default",
        &["agent", "start", "fake", "--workspace", workspace],
    );
    until(sandbox, &id_of(&started), "idle")
}

/// The trimmed lines of the terminal's screen.
fn screen(sandbox: &Sandbox, terminal: &str) -> Vec<String> {
    let screen = sandbox.json_ok("default", &["terminal", "screen", terminal]);
    screen["lines"]
        .as_array()
        .expect("screen lines")
        .iter()
        .filter_map(|line| line.as_str().map(|text| text.trim().to_owned()))
        .collect()
}

/// Waits until the terminal's screen shows `line`; returns the screen then.
fn wait_for_line(sandbox: &Sandbox, terminal: &str, line: &str) -> Vec<String> {
    eventually(&format!("`{line}` on the agent's screen"), || {
        let lines = screen(sandbox, terminal);
        lines.iter().any(|shown| shown == line).then_some(lines)
    })
}

#[test]
fn a_run_finds_massorca_first_on_its_path_and_gets_the_instructions_only_while_it_may_delegate() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let _service = spawn_service(&sandbox, &[]);
    folder_workspace(&sandbox);

    let parent = start_parent(&sandbox, "main");
    let lines = wait_for_line(
        &sandbox,
        &terminal_of(&parent),
        "fake-agent: path first massorca",
    );
    assert!(
        lines
            .iter()
            .any(|line| line == "fake-agent: instructions delegate"),
        "{lines:?}"
    );

    // The default depth of 1: the child cannot delegate, so it gets no text, while massorca is still on its PATH.
    let child = sandbox.json_ok(
        "default",
        &[
            "agent",
            "delegate",
            "--from",
            &id_of(&parent),
            "--prompt",
            "look",
            "--read-only",
        ],
    );
    let lines = wait_for_line(
        &sandbox,
        &terminal_of(&child),
        "fake-agent: path first massorca",
    );
    assert!(
        !lines
            .iter()
            .any(|line| line.starts_with("fake-agent: instructions")),
        "{lines:?}"
    );
}

#[test]
fn a_writing_child_of_a_folder_shares_it_and_so_does_any_other_writer() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let _service = spawn_service(&sandbox, &[]);
    folder_workspace(&sandbox);
    let parent = start_parent(&sandbox, "main");
    let workspaces = sandbox.json_ok("default", &["workspace", "list"])["workspaces"]
        .as_array()
        .expect("workspaces")
        .len();

    let child = sandbox.json_ok(
        "default",
        &[
            "agent",
            "delegate",
            "--from",
            &id_of(&parent),
            "--prompt",
            "write:notes.txt",
        ],
    );

    assert_eq!(child["read_only"], false, "{child}");
    assert_eq!(child["workspace_id"], parent["workspace_id"], "{child}");
    assert_eq!(child["note"], FOLDER_NOTE, "{child}");
    assert_eq!(child["notes"], json!([FOLDER_NOTE]), "{child}");
    let shown = sandbox.json_ok("default", &["agent", "status", &id_of(&child)]);
    assert_eq!(shown["notes"], json!([FOLDER_NOTE]), "{shown}");
    assert_eq!(
        sandbox.json_ok("default", &["workspace", "list"])["workspaces"]
            .as_array()
            .expect("workspaces")
            .len(),
        workspaces,
        "no copy and no new workspace"
    );
    assert!(parent.get("notes").is_none(), "{parent}");

    // A folder takes several writers (decision 0169): a writing start outside the tree is not refused, and it lists
    // both the parent and its child.
    let other = sandbox.json_ok(
        "default",
        &["agent", "start", "fake", "--workspace", "main"],
    );
    let shared = other["shared_folder_with"]
        .as_array()
        .expect("the folder's other writers");
    assert_eq!(shared.len(), 2, "{other}");
    assert!(shared.contains(&json!(id_of(&parent))), "{other}");
    assert!(shared.contains(&json!(id_of(&child))), "{other}");
}

#[test]
fn each_session_says_which_children_it_could_delegate() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let _service = spawn_service(&sandbox, &[]);
    folder_workspace(&sandbox);
    worktree_workspace(&sandbox);

    let lead = start_parent(&sandbox, "lead");
    assert_eq!(
        lead["delegation"],
        json!({ "writing_child": "own_worktree", "read_only_child": true }),
        "{lead}"
    );
    let folder = start_parent(&sandbox, "main");
    assert_eq!(
        folder["delegation"],
        json!({ "writing_child": "shared_folder", "read_only_child": true }),
        "{folder}"
    );

    // A worktree child gets its own worktree as before, and at the default depth of 1 it can delegate nothing.
    let child = sandbox.json_ok(
        "default",
        &[
            "agent",
            "delegate",
            "--from",
            &id_of(&lead),
            "--prompt",
            "look",
        ],
    );
    assert_ne!(child["workspace_id"], lead["workspace_id"], "{child}");
    assert!(child.get("notes").is_none(), "{child}");
    let deepest = &child["delegation"];
    assert_eq!(deepest["writing_child"], "unavailable", "{child}");
    assert_eq!(deepest["read_only_child"], false, "{child}");
    assert!(
        deepest["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("delegation depth limit")),
        "{child}"
    );
}

#[test]
fn a_cli_that_lists_the_channel_gets_the_text_and_is_probed_once() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let marker = sandbox.home().join("help-runs.txt");
    let _service = spawn_service(&sandbox, &[("FAKE_AGENT_HELP_MARKER", path_text(&marker))]);
    folder_workspace(&sandbox);

    let first = start_parent(&sandbox, "main");
    let second = sandbox.json_ok(
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
    for session in [&first, &second] {
        wait_for_line(
            &sandbox,
            &terminal_of(session),
            "fake-agent: instructions delegate",
        );
    }
    assert!(first.get("notes").is_none(), "{first}");
    let runs = fs::read_to_string(&marker).expect("the probe ran");
    assert_eq!(runs.lines().count(), 1, "one probe per installed program");
}

#[test]
fn a_cli_without_the_channel_starts_without_the_text_and_the_session_says_why() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let marker = sandbox.home().join("help-runs.txt");
    let _service = spawn_service(
        &sandbox,
        &[
            ("FAKE_AGENT_HELP_WITHOUT_INSTRUCTIONS", "1"),
            ("FAKE_AGENT_HELP_MARKER", path_text(&marker)),
        ],
    );
    folder_workspace(&sandbox);

    let parent = start_parent(&sandbox, "main");
    let lines = wait_for_line(
        &sandbox,
        &terminal_of(&parent),
        "fake-agent: path first massorca",
    );
    assert!(
        !lines
            .iter()
            .any(|line| line.starts_with("fake-agent: instructions")),
        "{lines:?}"
    );
    assert_eq!(
        parent["notes"],
        json!([
            "Delegation instructions were not given: this Fake agent CLI does not support --instructions."
        ]),
        "{parent}"
    );
    let second = sandbox.json_ok(
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
    assert_eq!(second["notes"], parent["notes"], "{second}");
    let runs = fs::read_to_string(&marker).expect("the probe ran");
    assert_eq!(runs.lines().count(), 1, "a failed probe is kept too");
}
