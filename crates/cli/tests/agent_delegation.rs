//! Delegation and bring-back through the CLI (decision 0055). A git project with a parent workspace `lead` runs the fake
//! agent; the tests wait on observed states (`agent status --until`) or on the agent's screen, never on sleeps.

mod harness;

use std::path::{Path, PathBuf};

use harness::{Sandbox, git, git_repo, single_document, with_json};
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
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join("fake.json"), kind.to_string()).expect("write a kind file");
}

/// A sandbox with kind `fake`, a git project `repo` whose workspaces go under `<home>/ws`, and its worktree workspace
/// `lead`. Returns the sandbox and the path of `lead`.
fn git_sandbox() -> (Sandbox, PathBuf) {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    // The service's merge commits need an identity; the repository's own config gives one on any machine.
    git(&repo, &["config", "user.name", "t"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
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

/// Starts the fake agent in workspace `lead` and waits for it to be idle; returns its session id and terminal.
fn start_parent(sandbox: &Sandbox, workspace: &str) -> (String, String) {
    let started = sandbox.json_ok(
        "default",
        &["agent", "start", "fake", "--workspace", workspace],
    );
    let session = started["id"].as_str().expect("session id").to_owned();
    let terminal = started["terminal_id"]
        .as_str()
        .expect("terminal id")
        .to_owned();
    until(sandbox, &session, "idle");
    (session, terminal)
}

/// `agent delegate --from <parent> --prompt <prompt> <extra>`, which must succeed; returns the child record.
fn delegate(sandbox: &Sandbox, parent: &str, prompt: &str, extra: &[&str]) -> Value {
    let mut args = vec!["agent", "delegate", "--from", parent, "--prompt", prompt];
    args.extend_from_slice(extra);
    sandbox.json_ok("default", &args)
}

fn id_of(record: &Value) -> String {
    record["id"].as_str().expect("session id").to_owned()
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

/// Runs `--json <args>`, which must fail; returns the error body.
fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure();
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

fn error_code(sandbox: &Sandbox, args: &[&str]) -> String {
    error_of(sandbox, args)["code"]
        .as_str()
        .expect("an error code")
        .to_owned()
}

/// The record of workspace `selector`.
fn workspace(sandbox: &Sandbox, selector: &str) -> Value {
    sandbox.json_ok("default", &["workspace", "show", selector])
}

fn workspace_count(sandbox: &Sandbox) -> usize {
    sandbox.json_ok("default", &["workspace", "list"])["workspaces"]
        .as_array()
        .expect("workspaces")
        .len()
}

fn head(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD"]).trim().to_owned()
}

#[test]
fn a_delegated_child_gets_its_own_workspace_on_a_branch_from_the_parent_and_shows_in_the_tree() {
    let (sandbox, lead) = git_sandbox();
    // The parent's branch moves past the project's HEAD, so the child must start from `lead`, not from the repository.
    git(&lead, &["commit", "-q", "--allow-empty", "-m", "lead only"]);
    let lead_tip = head(&lead);
    let (parent, _) = start_parent(&sandbox, "lead");

    let child = delegate(&sandbox, &parent, "look around", &[]);

    assert_eq!(child["parent_id"], parent.as_str(), "{child}");
    assert_eq!(child["kind"], "fake");
    assert!(
        child["note"]
            .as_str()
            .is_some_and(|note| note.contains("uncommitted")),
        "{child}"
    );
    let created = workspace(&sandbox, "lead-1");
    assert_eq!(child["workspace_id"], created["id"]);
    assert_eq!(created["branch"], "lead-1");
    let child_path = PathBuf::from(created["path"].as_str().expect("path"));
    assert_eq!(head(&child_path), lead_tip);
    let child_id = id_of(&child);
    let done = until(&sandbox, &child_id, "done");
    assert_eq!(done["result"], "done: look around");

    let tree = sandbox.json_ok("default", &["agent", "tree", &parent]);
    assert_eq!(tree["root"]["id"], parent.as_str());
    let children = tree["children"].as_array().expect("children");
    assert_eq!(children.len(), 1, "{tree}");
    assert_eq!(children[0]["id"], child_id.as_str());
    assert_eq!(children[0]["activity"]["state"], "done");
    assert_eq!(children[0]["result"], "done: look around");
    // Asked about the child, the tree still starts at its root.
    let from_child = sandbox.json_ok("default", &["agent", "tree", &child_id]);
    assert_eq!(from_child["root"]["id"], parent.as_str());
}

#[test]
fn a_read_only_child_runs_in_the_parent_workspace_and_creates_none() {
    let (sandbox, _lead) = git_sandbox();
    let (parent, _) = start_parent(&sandbox, "lead");
    let workspaces = workspace_count(&sandbox);

    let child = delegate(&sandbox, &parent, "read only", &["--read-only"]);

    assert_eq!(child["read_only"], true);
    assert_eq!(child["workspace_id"], workspace(&sandbox, "lead")["id"]);
    assert_eq!(workspace_count(&sandbox), workspaces);
    until(&sandbox, &id_of(&child), "done");
}

#[test]
fn a_child_cannot_delegate_and_a_wrong_token_is_refused() {
    let (sandbox, _lead) = git_sandbox();
    let (parent, _) = start_parent(&sandbox, "lead");
    let child = id_of(&delegate(&sandbox, &parent, "look", &[]));

    assert_eq!(
        error_code(
            &sandbox,
            &["agent", "delegate", "--from", &child, "--prompt", "deeper"]
        ),
        "agent.depth_exceeded"
    );

    // From inside a run, `--from` defaults to the run's session and the run's token goes with the call.
    let outcome = harness::execute(
        sandbox
            .command("default")
            .env("MASSORCA_AGENT_SESSION", &parent)
            .env("MASSORCA_AGENT_TOKEN", "not-the-token")
            .args(with_json(&["agent", "delegate", "--prompt", "sneaky"])),
    )
    .failure();
    assert_eq!(
        single_document(&outcome)["error"]["code"],
        "agent.hook_refused"
    );
}

#[test]
fn a_folder_parent_cannot_delegate_a_writer() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    let (parent, _) = start_parent(&sandbox, "main");

    assert_eq!(
        error_code(
            &sandbox,
            &["agent", "delegate", "--from", &parent, "--prompt", "write"]
        ),
        "agent.delegate_needs_git"
    );
}
