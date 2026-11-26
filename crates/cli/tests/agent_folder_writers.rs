//! Several writing sessions in one folder workspace (decision 0169), through the CLI with the fake agent declared as kind
//! `fake`. A folder has no git index, so the one-writer refusal covers worktrees only; a worktree keeps it. Every
//! session that writes in a folder lists the other live writers in `shared_folder_with`.
//!
//! Every CLI call is its own process, so a session that is still listed as live has outlived the call that started it.

mod harness;

use harness::{Sandbox, eventually, git, git_repo, single_document, with_json};
use serde_json::{Value, json};

const TEST_CHAT_KINDS_ENV: &str = "MASSORCA_TEST_CHAT_KINDS";

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
            "events": { "start": "idle", "prompt": "working", "stop": "done", "fail": "done:failure" },
            "result_field": "result"
        }
    });
    write_kind(sandbox, "fake.json", &kind);
}

/// Declares kind `fakechat`, which the test service starts as a chat run (`MASSORCA_TEST_CHAT_KINDS`).
fn declare_fakechat(sandbox: &Sandbox) {
    sandbox.set_env(TEST_CHAT_KINDS_ENV, "fakechat");
    let kind = json!({
        "id": "fakechat",
        "name": "Fake chat",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "none" }
    });
    write_kind(sandbox, "fakechat.json", &kind);
}

fn write_kind(sandbox: &Sandbox, file: &str, kind: &Value) {
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join(file), kind.to_string()).expect("write a kind file");
}

/// A sandbox with kinds `fake` and `fakechat` and a plain folder workspace `main`. Returns the sandbox.
fn folder_sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    declare_fakechat(&sandbox);
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create the folder");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    sandbox
}

/// A sandbox with kind `fake` and a git project `repo` whose worktree workspace is `lead`.
fn worktree_sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
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
    sandbox.json_ok("default", &["workspace", "create", "repo", "lead"]);
    sandbox
}

/// Starts `kind` in `workspace` with `extra` arguments, which must succeed; returns the session record.
fn start(sandbox: &Sandbox, kind: &str, workspace: &str, extra: &[&str]) -> Value {
    let mut args = vec!["agent", "start", kind, "--workspace", workspace];
    args.extend_from_slice(extra);
    sandbox.json_ok("default", &args)
}

/// The id of a session record.
fn id_of(record: &Value) -> String {
    record["id"].as_str().expect("session id").to_owned()
}

/// Runs `--json <args>`, which must fail, and returns the error document.
fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure();
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

/// The session's record, read again through its own command.
fn status(sandbox: &Sandbox, session: &str) -> Value {
    sandbox.json_ok("default", &["agent", "status", session])
}

/// Waits until the session's execution is `state`, then returns its record.
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
            "60",
        ],
    )
}

#[test]
fn two_writers_start_in_a_folder_and_each_lists_the_other() {
    let sandbox = folder_sandbox();
    let first = start(&sandbox, "fake", "main", &[]);
    let second = start(&sandbox, "fake", "main", &[]);

    assert_eq!(second["execution"]["state"], "live", "{second}");
    assert_eq!(
        second["shared_folder_with"],
        json!([id_of(&first)]),
        "{second}"
    );
    assert_eq!(
        status(&sandbox, &id_of(&first))["shared_folder_with"],
        json!([id_of(&second)]),
        "the first writer now lists the second"
    );
    let listed = sandbox.json_ok("default", &["agent", "list", "--workspace", "main"]);
    for session in listed["sessions"].as_array().expect("sessions") {
        assert_eq!(session["execution"]["state"], "live", "{session}");
        assert_eq!(
            session["shared_folder_with"].as_array().map(Vec::len),
            Some(1)
        );
    }
}

#[test]
fn a_session_that_exits_leaves_the_others_list() {
    let sandbox = folder_sandbox();
    let first = start(&sandbox, "fake", "main", &[]);
    let second = start(&sandbox, "fake", "main", &[]);
    sandbox.json_ok("default", &["agent", "stop", &id_of(&second), "--force"]);

    let after = status(&sandbox, &id_of(&first));
    assert_eq!(after["execution"]["state"], "live", "{after}");
    assert!(
        after.get("shared_folder_with").is_none(),
        "an exited session is not listed: {after}"
    );
}

#[test]
fn a_read_only_session_neither_refuses_a_writer_nor_is_listed() {
    let sandbox = folder_sandbox();
    let writer = start(&sandbox, "fake", "main", &[]);
    let reader = start(&sandbox, "fake", "main", &["--read-only"]);
    assert_eq!(reader["read_only"], true, "{reader}");
    assert!(reader.get("shared_folder_with").is_none(), "{reader}");

    let second_writer = start(&sandbox, "fake", "main", &[]);
    assert_eq!(
        second_writer["shared_folder_with"],
        json!([id_of(&writer)]),
        "the read-only session is not a writer: {second_writer}"
    );
}

#[test]
fn a_resume_runs_while_another_session_writes_in_a_folder() {
    let sandbox = folder_sandbox();
    let first = start(&sandbox, "fake", "main", &[]);
    until(&sandbox, &id_of(&first), "idle");
    sandbox.json_ok("default", &["agent", "stop", &id_of(&first), "--force"]);
    let other = start(&sandbox, "fake", "main", &[]);

    let resumed = sandbox.json_ok("default", &["agent", "resume", &id_of(&first)]);
    assert_eq!(resumed["execution"]["state"], "live", "{resumed}");
    assert_eq!(
        resumed["shared_folder_with"],
        json!([id_of(&other)]),
        "{resumed}"
    );
}

#[test]
fn a_chat_start_in_a_folder_runs_beside_a_live_terminal_writer() {
    let sandbox = folder_sandbox();
    let writer = start(&sandbox, "fake", "main", &[]);
    let chat = start(&sandbox, "fakechat", "main", &[]);

    assert_eq!(chat["run"], "chat", "{chat}");
    assert_eq!(
        chat["shared_folder_with"],
        json!([id_of(&writer)]),
        "{chat}"
    );
}

#[test]
fn a_second_writer_in_a_worktree_is_still_refused() {
    let sandbox = worktree_sandbox();
    let first = start(&sandbox, "fake", "lead", &[]);
    until(&sandbox, &id_of(&first), "idle");

    let error = error_of(&sandbox, &["agent", "start", "fake", "--workspace", "lead"]);
    assert_eq!(error["code"], "agent.workspace_busy", "{error}");
    assert_eq!(error["data"]["session"], id_of(&first).as_str(), "{error}");
    assert!(
        status(&sandbox, &id_of(&first))
            .get("shared_folder_with")
            .is_none(),
        "a worktree never lists a second writer"
    );
}

#[test]
fn a_read_only_start_in_a_worktree_is_unaffected() {
    let sandbox = worktree_sandbox();
    let first = start(&sandbox, "fake", "lead", &[]);
    until(&sandbox, &id_of(&first), "idle");

    let reader = start(&sandbox, "fake", "lead", &["--read-only"]);
    assert_eq!(reader["read_only"], true, "{reader}");
    eventually("the read-only session is live", || {
        (status(&sandbox, &id_of(&reader))["execution"]["state"] == "live").then_some(())
    });
}

#[test]
fn a_resume_in_a_worktree_is_still_refused_while_another_writes() {
    let sandbox = worktree_sandbox();
    let first = start(&sandbox, "fake", "lead", &[]);
    until(&sandbox, &id_of(&first), "idle");
    sandbox.json_ok("default", &["agent", "stop", &id_of(&first), "--force"]);
    start(&sandbox, "fake", "lead", &[]);

    let error = error_of(&sandbox, &["agent", "resume", &id_of(&first)]);
    assert_eq!(error["code"], "agent.workspace_busy", "{error}");
}
