//! Hiding (archiving) and deleting agent sessions through the CLI (decision 0163): `agent hide`, `agent list --archived`
//! and `agent forget --preview | --remove-workspace [--force]`. The fake agent runs in a git project's worktree
//! workspaces or in a plain folder; the tests wait on observed states, never on sleeps.

mod harness;

use std::path::{Path, PathBuf};

use harness::{Sandbox, git, git_repo, single_document, with_json};
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

/// Creates worktree workspace `name` of project `repo`; returns its record.
fn worktree(sandbox: &Sandbox, name: &str) -> Value {
    sandbox.json_ok("default", &["workspace", "create", "repo", name])
}

/// Registers `<home>/notes` as a folder project and creates its workspace `main`; returns its record.
fn folder(sandbox: &Sandbox) -> Value {
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"])
}

fn text<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("no {field} in {value}"))
}

/// Starts the fake agent in `workspace` (with `extra` arguments) and waits for it to be idle; returns its session id.
fn start(sandbox: &Sandbox, workspace: &str, extra: &[&str]) -> String {
    let mut args = vec!["agent", "start", "fake", "--workspace", workspace];
    args.extend_from_slice(extra);
    let started = sandbox.json_ok("default", &args);
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

/// Starts a session in `workspace` and stops it; returns its id.
fn exited(sandbox: &Sandbox, workspace: &str, extra: &[&str]) -> String {
    let session = start(sandbox, workspace, extra);
    sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);
    session
}

/// Runs `--json <args>`, which must fail; returns the error body.
fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure();
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

fn error_code(sandbox: &Sandbox, args: &[&str]) -> String {
    text(&error_of(sandbox, args), "code").to_owned()
}

/// The ids `agent list` prints, with `extra` arguments.
fn listed(sandbox: &Sandbox, extra: &[&str]) -> Vec<String> {
    let mut args = vec!["agent", "list"];
    args.extend_from_slice(extra);
    sandbox.json_ok("default", &args)["sessions"]
        .as_array()
        .expect("sessions")
        .iter()
        .map(|session| text(session, "id").to_owned())
        .collect()
}

fn workspace_exists(sandbox: &Sandbox, id: &str) -> bool {
    sandbox
        .run("default", &with_json(&["workspace", "show", id]))
        .get_output()
        .status
        .success()
}

fn branch_exists(repo: &Path, branch: &str) -> bool {
    !git(repo, &["branch", "--list", branch]).trim().is_empty()
}

fn repo_of(sandbox: &Sandbox) -> PathBuf {
    sandbox.home().join("repo")
}

#[test]
fn a_hidden_session_leaves_the_list_until_asked_for_and_a_live_one_cannot_be_hidden() {
    let sandbox = git_sandbox();
    worktree(&sandbox, "w1");
    let session = start(&sandbox, "w1", &[]);

    assert_eq!(
        error_code(&sandbox, &["agent", "hide", &session]),
        "agent.busy",
        "a live run is refused"
    );
    sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);

    let hidden = sandbox.json_ok("default", &["agent", "hide", &session]);
    assert_eq!(hidden["archived"], true, "{hidden}");
    assert!(
        listed(&sandbox, &[]).is_empty(),
        "a hidden session is not listed"
    );
    assert_eq!(
        listed(&sandbox, &["--archived"]),
        std::slice::from_ref(&session)
    );
    sandbox
        .run("default", &["agent", "list", "--archived"])
        .success()
        .stdout(contains("archived"));
    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(shown["archived"], true, "status still finds it");

    sandbox.run("default", &["service", "restart"]).success();
    assert_eq!(
        listed(&sandbox, &["--archived"]),
        std::slice::from_ref(&session),
        "hiding is durable"
    );
    assert!(listed(&sandbox, &[]).is_empty());

    let shown_again = sandbox.json_ok("default", &["agent", "hide", &session, "--off"]);
    assert_ne!(shown_again["archived"], true, "{shown_again}");
    assert_eq!(listed(&sandbox, &[]), [session]);
}

#[test]
fn a_hidden_session_keeps_its_terminal_output() {
    let sandbox = git_sandbox();
    worktree(&sandbox, "w1");
    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            "w1",
            "--prompt",
            "remember me",
        ],
    );
    let session = text(&started, "id").to_owned();
    let terminal = text(&started, "terminal_id").to_owned();
    sandbox.json_ok(
        "default",
        &[
            "agent",
            "status",
            &session,
            "--until",
            "done",
            "--timeout",
            UNTIL_SECS,
        ],
    );
    sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);
    sandbox.json_ok("default", &["agent", "hide", &session]);
    let screen = sandbox.json_ok("default", &["terminal", "screen", &terminal]);
    assert!(
        screen.to_string().contains("fake-agent: done remember me"),
        "{screen}"
    );
}

#[test]
fn the_preview_of_a_worktree_session_reports_dirt_other_sessions_and_whether_it_is_removable() {
    let sandbox = git_sandbox();
    let created = worktree(&sandbox, "w1");
    let workspace_id = text(&created, "id").to_owned();
    let path = PathBuf::from(text(&created, "path"));
    let writer = exited(&sandbox, "w1", &[]);

    let clean = sandbox.json_ok("default", &["agent", "forget", &writer, "--preview"]);
    assert_eq!(clean["session"], writer.as_str());
    assert_eq!(clean["can_forget"], true, "{clean}");
    let ws = &clean["workspace"];
    assert_eq!(ws["id"], workspace_id.as_str());
    assert_eq!(ws["kind"], "worktree");
    assert_eq!(ws["removable"], true, "{clean}");
    assert_eq!(ws["dirty"], false);
    assert_eq!(ws["untracked"], false);
    assert_eq!(ws["other_sessions"], json!([]));
    assert_eq!(ws["branch"], created["branch"]);
    sandbox
        .run("default", &["agent", "forget", &writer, "--preview"])
        .success()
        .stdout(contains("removable: yes"));

    std::fs::write(path.join("README.md"), "changed\n").expect("change a tracked file");
    std::fs::write(path.join("new.txt"), "new\n").expect("add an untracked file");
    let dirty = sandbox.json_ok("default", &["agent", "forget", &writer, "--preview"]);
    let ws = &dirty["workspace"];
    assert_eq!(ws["dirty"], true, "{dirty}");
    assert_eq!(ws["untracked"], true, "{dirty}");
    assert_eq!(ws["removable"], false, "{dirty}");
    assert!(text(ws, "blocker").contains("uncommitted"), "{dirty}");

    let reader = exited(&sandbox, "w1", &["--read-only"]);
    let shared = sandbox.json_ok("default", &["agent", "forget", &writer, "--preview"]);
    assert_eq!(shared["workspace"]["other_sessions"], json!([reader]));
    assert_eq!(shared["workspace"]["removable"], false);

    let live = start(&sandbox, "w1", &["--read-only"]);
    let busy = sandbox.json_ok("default", &["agent", "forget", &live, "--preview"]);
    assert_eq!(busy["can_forget"], false, "{busy}");
    assert!(text(&busy, "forget_blocker").contains("stop it first"));
    assert!(path.is_dir(), "a preview changes nothing");
    assert_eq!(listed(&sandbox, &[]).len(), 3);
}

#[test]
fn the_preview_of_a_folder_session_says_its_files_stay() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let created = folder(&sandbox);
    let session = exited(&sandbox, "main", &[]);
    let preview = sandbox.json_ok("default", &["agent", "forget", &session, "--preview"]);
    let ws = &preview["workspace"];
    assert_eq!(ws["id"], created["id"]);
    assert_eq!(ws["kind"], "folder");
    assert_eq!(ws["removable"], true, "{preview}");
    assert_eq!(ws["dirty"], false);

    let forgotten = sandbox.json_ok(
        "default",
        &["agent", "forget", &session, "--remove-workspace"],
    );
    assert_eq!(forgotten["id"], session.as_str());
    assert_eq!(forgotten["workspace_removed"]["directory_kept"], true);
    assert!(!workspace_exists(&sandbox, text(&created, "id")));
    assert!(
        sandbox.home().join("notes").is_dir(),
        "a folder's files stay"
    );
}

#[test]
fn forget_with_remove_workspace_removes_the_worktree_and_its_created_branch() {
    let sandbox = git_sandbox();
    let created = worktree(&sandbox, "w1");
    let workspace_id = text(&created, "id").to_owned();
    let path = PathBuf::from(text(&created, "path"));
    let branch = text(&created, "branch").to_owned();
    let session = exited(&sandbox, "w1", &[]);
    sandbox.json_ok("default", &["agent", "hide", &session]);
    assert!(branch_exists(&repo_of(&sandbox), &branch));

    let forgotten = sandbox.json_ok(
        "default",
        &["agent", "forget", &session, "--remove-workspace"],
    );
    assert_eq!(forgotten["id"], session.as_str());
    assert_eq!(
        forgotten["workspace_removed"]["workspace"]["id"],
        workspace_id.as_str()
    );
    assert_eq!(forgotten["workspace_removed"]["branch_deleted"], true);
    assert!(forgotten.get("workspace_error").is_none(), "{forgotten}");
    assert!(!path.exists(), "the worktree is gone");
    assert!(!branch_exists(&repo_of(&sandbox), &branch));
    assert!(!workspace_exists(&sandbox, &workspace_id));
    assert_eq!(
        error_code(&sandbox, &["agent", "status", &session]),
        "agent.not_found"
    );
}

#[test]
fn forget_with_remove_workspace_is_refused_while_another_session_uses_the_workspace() {
    let sandbox = git_sandbox();
    let created = worktree(&sandbox, "w1");
    let workspace_id = text(&created, "id").to_owned();
    let path = PathBuf::from(text(&created, "path"));
    let writer = exited(&sandbox, "w1", &[]);
    let reader = exited(&sandbox, "w1", &["--read-only"]);
    sandbox.json_ok("default", &["agent", "hide", &reader]);

    let error = error_of(
        &sandbox,
        &["agent", "forget", &writer, "--remove-workspace", "--force"],
    );
    assert_eq!(error["code"], "agent.busy");
    assert!(text(&error, "message").contains(&reader), "{error}");
    assert!(path.is_dir(), "nothing was deleted");
    assert!(workspace_exists(&sandbox, &workspace_id));
    let shown = sandbox.json_ok("default", &["agent", "status", &writer]);
    assert_eq!(shown["id"], writer.as_str(), "the session is kept");
}

#[test]
fn a_dirty_worktree_is_removed_only_with_force() {
    let sandbox = git_sandbox();
    let created = worktree(&sandbox, "w1");
    let workspace_id = text(&created, "id").to_owned();
    let path = PathBuf::from(text(&created, "path"));
    let session = exited(&sandbox, "w1", &[]);
    std::fs::write(path.join("README.md"), "changed\n").expect("change a tracked file");

    assert_eq!(
        error_code(
            &sandbox,
            &["agent", "forget", &session, "--remove-workspace"]
        ),
        "workspace.dirty"
    );
    assert!(path.is_dir(), "nothing was deleted");
    assert!(workspace_exists(&sandbox, &workspace_id));
    sandbox.json_ok("default", &["agent", "status", &session]);

    let forgotten = sandbox.json_ok(
        "default",
        &["agent", "forget", &session, "--remove-workspace", "--force"],
    );
    assert_eq!(forgotten["id"], session.as_str());
    assert!(!path.exists(), "the forced removal discards the changes");
    assert!(!workspace_exists(&sandbox, &workspace_id));
}

#[test]
fn a_plain_forget_keeps_the_workspace() {
    let sandbox = git_sandbox();
    let created = worktree(&sandbox, "w1");
    let session = exited(&sandbox, "w1", &[]);
    let forgotten = sandbox.json_ok("default", &["agent", "forget", &session]);
    assert_eq!(forgotten["id"], session.as_str());
    assert!(forgotten.get("workspace_removed").is_none(), "{forgotten}");
    assert!(workspace_exists(&sandbox, text(&created, "id")));
    assert!(PathBuf::from(text(&created, "path")).is_dir());
}

#[test]
fn a_session_is_renamed_live_or_exited_and_a_bad_title_is_refused() {
    let sandbox = git_sandbox();
    worktree(&sandbox, "w1");
    let session = start(&sandbox, "w1", &[]);

    let renamed = sandbox.json_ok("default", &["agent", "rename", &session, "  Live name  "]);
    assert_eq!(
        renamed["title"], "Live name",
        "a live session is renamed, trimmed"
    );
    sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);
    let again = sandbox.json_ok("default", &["agent", "rename", &session, "Exited name"]);
    assert_eq!(again["title"], "Exited name");
    sandbox
        .run("default", &["agent", "rename", &session, "Text name"])
        .success()
        .stdout(contains("title: Text name"));

    let long = "x".repeat(121);
    for bad in ["   ", long.as_str(), "tab\there"] {
        assert_eq!(
            error_code(&sandbox, &["agent", "rename", &session, bad]),
            "request.invalid_params",
            "{bad:?}"
        );
    }
    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(shown["title"], "Text name", "a refusal changes nothing");
}

#[test]
fn ignored_files_refuse_the_removal_unless_discarded() {
    let sandbox = git_sandbox();
    let created = worktree(&sandbox, "w1");
    let path = PathBuf::from(text(&created, "path"));
    let session = exited(&sandbox, "w1", &[]);
    // The repository's own exclude file is shared by its worktrees, so `build/` is ignored without a tracked change.
    std::fs::write(repo_of(&sandbox).join(".git/info/exclude"), "build/\n").expect("write exclude");
    std::fs::create_dir_all(path.join("build")).expect("create build dir");
    std::fs::write(path.join("build/out.bin"), "built\n").expect("write build output");

    let preview = sandbox.json_ok("default", &["agent", "forget", &session, "--preview"]);
    assert_eq!(preview["workspace"]["ignored_files"], true, "{preview}");
    assert_eq!(
        error_code(
            &sandbox,
            &["agent", "forget", &session, "--remove-workspace", "--force"]
        ),
        "workspace.ignored_files",
        "force alone does not discard ignored files"
    );
    assert!(path.is_dir(), "nothing was deleted");
    let forgotten = sandbox.json_ok(
        "default",
        &[
            "agent",
            "forget",
            &session,
            "--remove-workspace",
            "--discard-ignored",
        ],
    );
    assert_eq!(forgotten["id"], session.as_str());
    assert!(forgotten.get("workspace_error").is_none(), "{forgotten}");
    assert!(!path.exists());
}

#[test]
fn a_running_terminal_in_the_workspace_refuses_the_removal_and_keeps_running() {
    let sandbox = git_sandbox();
    let created = worktree(&sandbox, "w1");
    let path = PathBuf::from(text(&created, "path"));
    let session = exited(&sandbox, "w1", &[]);
    let terminal = sandbox.json_ok("default", &["terminal", "create", "w1"]);
    let terminal_id = text(&terminal, "id").to_owned();

    let preview = sandbox.json_ok("default", &["agent", "forget", &session, "--preview"]);
    assert_eq!(
        preview["workspace"]["running_terminals"],
        json!([terminal_id])
    );
    assert_eq!(preview["workspace"]["removable"], false);
    assert_eq!(
        error_code(
            &sandbox,
            &["agent", "forget", &session, "--remove-workspace"]
        ),
        "workspace.busy"
    );
    assert!(path.is_dir(), "nothing was deleted");
    let shown = sandbox.json_ok("default", &["terminal", "show", &terminal_id]);
    assert!(shown["exit"].is_null(), "the terminal still runs: {shown}");
    sandbox.json_ok("default", &["agent", "status", &session]);
}
