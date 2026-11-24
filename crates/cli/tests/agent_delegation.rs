//! Delegation and bring-back through the CLI (decision 0055). A git project with a parent workspace `lead` runs the fake
//! agent; the tests wait on observed states (`agent status --until`) or on the agent's screen, never on sleeps.

mod harness;

use std::path::{Path, PathBuf};

use harness::{Sandbox, eventually, git, git_repo, single_document, with_json};
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

/// Waits until a line of the terminal's screen contains `text`.
fn wait_for_text(sandbox: &Sandbox, terminal: &str, text: &str) {
    eventually(&format!("`{text}` on the agent's screen"), || {
        let screen = sandbox.json_ok("default", &["terminal", "screen", terminal]);
        screen["lines"]
            .as_array()
            .is_some_and(|lines| {
                lines
                    .iter()
                    .any(|shown| shown.as_str().is_some_and(|line| line.contains(text)))
            })
            .then_some(())
    });
}

/// Delegates `prompt` to a writing child of `parent` and waits until the child is done; returns its id.
fn finished_child(sandbox: &Sandbox, parent: &str, prompt: &str) -> String {
    let child = id_of(&delegate(sandbox, parent, prompt, &[]));
    until(sandbox, &child, "done");
    child
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
fn bring_back_merges_the_child_branch_with_a_merge_commit_and_prompts_the_parent_once() {
    let (sandbox, lead) = git_sandbox();
    let (parent, terminal) = start_parent(&sandbox, "lead");
    let before = head(&lead);
    let child = finished_child(&sandbox, &parent, "write:result.txt");

    let brought = sandbox.json_ok("default", &["agent", "bring-back", &child]);

    assert_eq!(brought["merged"], true, "{brought}");
    assert_eq!(brought["prompt_delivered"], true, "{brought}");
    let merge = brought["merge_commit"].as_str().expect("merge commit");
    assert_eq!(head(&lead), merge);
    let parents = git(&lead, &["rev-list", "--parents", "-n", "1", "HEAD"]);
    let ids: Vec<&str> = parents.split_whitespace().collect();
    assert_eq!(
        ids.len(),
        3,
        "a --no-ff merge commit has two parents: {parents}"
    );
    assert_eq!(ids[1], before);
    assert!(lead.join("result.txt").is_file());
    assert!(brought["child"]["brought_back_at_ms"].is_i64(), "{brought}");
    wait_for_text(
        &sandbox,
        &terminal,
        "working on MassOrca: delegated session",
    );

    assert_eq!(
        error_code(&sandbox, &["agent", "bring-back", &child]),
        "agent.already_brought_back"
    );
}

#[test]
fn bring_back_of_a_working_child_is_busy() {
    let (sandbox, _lead) = git_sandbox();
    let (parent, _) = start_parent(&sandbox, "lead");
    let child = id_of(&delegate(&sandbox, &parent, "hold on", &[]));
    until(&sandbox, &child, "working");

    assert_eq!(
        error_code(&sandbox, &["agent", "bring-back", &child]),
        "agent.busy"
    );
}

#[test]
fn bring_back_into_a_dirty_parent_is_refused_and_changes_nothing() {
    let (sandbox, lead) = git_sandbox();
    let (parent, _) = start_parent(&sandbox, "lead");
    let child = finished_child(&sandbox, &parent, "write:result.txt");
    std::fs::write(lead.join("scratch.txt"), "local\n").expect("write a local file");
    let before = head(&lead);

    assert_eq!(
        error_code(&sandbox, &["agent", "bring-back", &child]),
        "agent.parent_dirty"
    );
    assert_eq!(head(&lead), before);
    assert!(!lead.join("result.txt").exists());
    assert!(lead.join("scratch.txt").is_file());
    // Not marked: once the parent is clean the same child comes back.
    std::fs::remove_file(lead.join("scratch.txt")).expect("remove the local file");
    let brought = sandbox.json_ok("default", &["agent", "bring-back", &child, "--no-prompt"]);
    assert_eq!(brought["merged"], true, "{brought}");
}

#[test]
fn a_conflicting_bring_back_lists_the_paths_and_leaves_the_parent_as_it_was() {
    let (sandbox, lead) = git_sandbox();
    let (parent, _) = start_parent(&sandbox, "lead");
    let child = finished_child(&sandbox, &parent, "write:README.md");
    std::fs::write(lead.join("README.md"), "from the parent\n").expect("write README.md");
    git(&lead, &["commit", "-q", "-am", "parent change"]);
    let before = head(&lead);

    let error = error_of(&sandbox, &["agent", "bring-back", &child]);

    assert_eq!(error["code"], "agent.merge_conflict", "{error}");
    assert_eq!(error["data"]["paths"], json!(["README.md"]), "{error}");
    assert_eq!(head(&lead), before);
    assert_eq!(
        // A checkout with `core.autocrlf` writes the file back with CRLF line ends.
        std::fs::read_to_string(lead.join("README.md"))
            .expect("read README.md")
            .replace("\r\n", "\n"),
        "from the parent\n"
    );
    assert!(git(&lead, &["status", "--porcelain"]).is_empty());
    let merging = git(&lead, &["rev-parse", "--git-path", "MERGE_HEAD"]);
    assert!(!lead.join(merging.trim()).exists());
    let shown = sandbox.json_ok("default", &["agent", "status", &child]);
    assert!(shown.get("brought_back_at_ms").is_none(), "{shown}");
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

#[test]
fn no_merge_and_no_prompt_each_skip_their_step() {
    let (sandbox, lead) = git_sandbox();
    let (parent, terminal) = start_parent(&sandbox, "lead");
    let unmerged = finished_child(&sandbox, &parent, "write:first.txt");
    let unprompted = finished_child(&sandbox, &parent, "write:second.txt");
    let before = head(&lead);

    let brought = sandbox.json_ok(
        "default",
        &["agent", "bring-back", &unprompted, "--no-prompt"],
    );
    assert_eq!(brought["merged"], true, "{brought}");
    assert_eq!(brought["prompt_delivered"], false, "{brought}");
    assert!(lead.join("second.txt").is_file());
    let shown = sandbox.json_ok("default", &["agent", "status", &parent]);
    assert_eq!(shown["activity"]["state"], "idle", "{shown}");

    let brought = sandbox.json_ok("default", &["agent", "bring-back", &unmerged, "--no-merge"]);
    assert_eq!(brought["merged"], false, "{brought}");
    assert!(brought.get("merge_commit").is_none(), "{brought}");
    assert_eq!(brought["prompt_delivered"], true, "{brought}");
    wait_for_text(
        &sandbox,
        &terminal,
        "working on MassOrca: delegated session",
    );
    // The fake parent acts on the words of the prompt, so it may commit files of its own; what must hold is that the
    // child's branch was not merged.
    until(&sandbox, &parent, "done");
    assert!(!is_ancestor(&lead, "lead-1", "HEAD"));
    assert!(is_ancestor(&lead, "lead-2", "HEAD"));
    assert_ne!(head(&lead), before);
}

/// Whether `ancestor` is an ancestor of `of` in the repository at `dir`.
fn is_ancestor(dir: &Path, ancestor: &str, of: &str) -> bool {
    let status = massorca_process::command("git")
        .args(["merge-base", "--is-ancestor", ancestor, of])
        .current_dir(dir)
        .status()
        .expect("spawn git");
    match status.code() {
        Some(0) => true,
        Some(1) => false,
        other => panic!("git merge-base --is-ancestor failed: {other:?}"),
    }
}

/// Runs `--json <args>` as an agent run would, with the run's session and token in the environment; it must fail.
/// Returns the error code.
fn error_inside_run(sandbox: &Sandbox, session: &str, token: &str, args: &[&str]) -> String {
    let outcome = harness::execute(
        sandbox
            .command("default")
            .env("MASSORCA_AGENT_SESSION", session)
            .env("MASSORCA_AGENT_TOKEN", token)
            .args(with_json(args)),
    )
    .failure();
    single_document(&outcome)["error"]["code"]
        .as_str()
        .expect("an error code")
        .to_owned()
}

#[test]
fn a_child_cannot_act_for_its_parent_from_inside_its_run() {
    let (sandbox, lead) = git_sandbox();
    let (parent, _) = start_parent(&sandbox, "lead");
    let child = finished_child(&sandbox, &parent, "write:result.txt");
    let before = head(&lead);

    // Inside its own run the child's token goes with every call, and it is not the parent's.
    assert_eq!(
        error_inside_run(
            &sandbox,
            &child,
            "the-childs-token",
            &["agent", "delegate", "--from", &parent, "--prompt", "sneaky"]
        ),
        "agent.hook_refused"
    );
    assert_eq!(
        error_inside_run(
            &sandbox,
            &child,
            "the-childs-token",
            &["agent", "bring-back", &child]
        ),
        "agent.hook_refused"
    );

    assert_eq!(head(&lead), before);
    let tree = sandbox.json_ok("default", &["agent", "tree", &parent]);
    assert_eq!(tree["children"].as_array().map(Vec::len), Some(1), "{tree}");
    let shown = sandbox.json_ok("default", &["agent", "status", &child]);
    assert!(shown.get("brought_back_at_ms").is_none(), "{shown}");
}

#[test]
fn bring_back_into_a_parent_with_a_stopped_rebase_is_refused() {
    let (sandbox, lead) = git_sandbox();
    let (parent, _) = start_parent(&sandbox, "lead");
    let child = finished_child(&sandbox, &parent, "write:result.txt");
    git(&lead, &["commit", "-q", "--allow-empty", "-m", "second"]);
    // An exec that fails stops the rebase and leaves the work tree clean.
    let stopped = massorca_process::command("git")
        .args([
            "-c",
            "core.editor=true",
            "rebase",
            "--exec",
            "exit 1",
            "HEAD~1",
        ])
        .current_dir(&lead)
        .output()
        .expect("spawn git");
    assert!(!stopped.status.success());
    assert!(git(&lead, &["status", "--porcelain"]).is_empty());
    let before = head(&lead);

    let error = error_of(&sandbox, &["agent", "bring-back", &child]);

    assert_eq!(error["code"], "agent.parent_dirty", "{error}");
    assert!(
        error["message"]
            .as_str()
            .is_some_and(|message| message.contains("rebase")),
        "{error}"
    );
    assert_eq!(head(&lead), before);
    assert!(!lead.join("result.txt").exists());
}

#[test]
fn bring_back_while_another_session_writes_in_the_parent_workspace_is_refused() {
    let (sandbox, lead) = git_sandbox();
    let (parent, _) = start_parent(&sandbox, "lead");
    let child = finished_child(&sandbox, &parent, "write:result.txt");
    sandbox.json_ok("default", &["agent", "stop", &parent, "--force"]);
    // With the parent stopped, another writer takes the parent's workspace.
    let (other, _) = start_parent(&sandbox, "lead");
    let before = head(&lead);

    let error = error_of(&sandbox, &["agent", "bring-back", &child]);

    assert_eq!(error["code"], "agent.workspace_busy", "{error}");
    assert_eq!(error["data"]["session"], other.as_str(), "{error}");
    assert_eq!(head(&lead), before);
    assert!(!lead.join("result.txt").exists());
}

/// Sets `agents.delegation_depth` through `config set`, which refuses a value outside its range.
fn set_delegation_depth(sandbox: &Sandbox, depth: u32) {
    let text = depth.to_string();
    sandbox.json_ok(
        "default",
        &["config", "set", "agents.delegation_depth", text.as_str()],
    );
}

#[test]
fn a_grandchild_is_started_under_its_child_and_comes_back_to_it_before_the_root() {
    let (sandbox, lead) = git_sandbox();
    set_delegation_depth(&sandbox, 2);
    let (parent, _) = start_parent(&sandbox, "lead");
    let child = finished_child(&sandbox, &parent, "write:child.txt");
    let grandchild = id_of(&delegate(&sandbox, &child, "write:deep.txt", &[]));
    let started = until(&sandbox, &grandchild, "done");

    assert_eq!(started["parent_id"], child.as_str(), "{started}");
    assert_eq!(
        workspace(&sandbox, "lead-1-1")["id"],
        started["workspace_id"]
    );
    let tree = sandbox.json_ok("default", &["agent", "tree", &grandchild]);
    assert_eq!(tree["root"]["id"], parent.as_str(), "{tree}");
    assert_eq!(tree["children"].as_array().map(Vec::len), Some(1), "{tree}");

    let brought = sandbox.json_ok(
        "default",
        &["agent", "bring-back", &grandchild, "--no-prompt"],
    );
    assert_eq!(brought["merged"], true, "{brought}");
    let child_workspace = PathBuf::from(workspace(&sandbox, "lead-1")["path"].as_str().unwrap());
    assert!(child_workspace.join("deep.txt").is_file());
    assert!(
        !lead.join("deep.txt").exists(),
        "the root must not see the grandchild yet"
    );

    let brought = sandbox.json_ok("default", &["agent", "bring-back", &child, "--no-prompt"]);
    assert_eq!(brought["merged"], true, "{brought}");
    assert!(lead.join("deep.txt").is_file());
    assert!(lead.join("child.txt").is_file());
}

#[test]
fn a_grandchild_cannot_delegate_past_the_configured_limit_and_the_refusal_names_it() {
    let (sandbox, _lead) = git_sandbox();
    set_delegation_depth(&sandbox, 2);
    let (parent, _) = start_parent(&sandbox, "lead");
    let child = id_of(&delegate(&sandbox, &parent, "look", &[]));
    let grandchild = id_of(&delegate(&sandbox, &child, "look", &[]));

    let error = error_of(
        &sandbox,
        &[
            "agent",
            "delegate",
            "--from",
            &grandchild,
            "--prompt",
            "deeper",
        ],
    );

    assert_eq!(error["code"], "agent.depth_exceeded", "{error}");
    let message = error["message"].as_str().expect("a message");
    assert!(message.contains("agents.delegation_depth"), "{error}");
    assert!(message.contains('2'), "{error}");
    assert_eq!(error["data"]["limit"], 2, "{error}");
    assert_eq!(
        error["data"]["setting"], "agents.delegation_depth",
        "{error}"
    );
}

#[test]
fn stopping_the_root_stops_its_grandchild_as_well() {
    let (sandbox, _lead) = git_sandbox();
    set_delegation_depth(&sandbox, 2);
    let (parent, _) = start_parent(&sandbox, "lead");
    let child = id_of(&delegate(&sandbox, &parent, "look", &[]));
    let grandchild = id_of(&delegate(&sandbox, &child, "hold on", &[]));
    until(&sandbox, &grandchild, "working");

    sandbox.json_ok("default", &["agent", "stop", &parent, "--force"]);

    for session in [&child, &grandchild] {
        eventually(&format!("session {session} has exited"), || {
            let shown = sandbox.json_ok("default", &["agent", "status", session]);
            (shown["execution"]["state"] == "exited").then_some(())
        });
    }
}
