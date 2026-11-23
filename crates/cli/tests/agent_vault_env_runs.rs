//! Vault secrets on the other launch paths (decision 0071): a resume of a locked vault is refused and leaves the
//! session as it was, and a delegated child receives its project's secrets. Every value here is a placeholder.

mod harness;

use std::fs;
use std::process::Stdio;

use harness::{ChildGuard, Sandbox, eventually, massorca_in, single_document, with_json};
use serde_json::{Value, json};

const UNTIL_SECS: &str = "60";

/// Kind `fake`: argv prompt, an assigned ref, a resume form, and status reports.
fn declare_fake(sandbox: &Sandbox) {
    // The kind file goes into the profile, so the profile is created private first (ADR 0073).
    sandbox.prepare_profile("default");
    let kind = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
        "resume": ["--resume", "{ref}"],
        "status": {
            "mechanism": "env-command",
            "events": { "start": "idle", "prompt": "working", "stop": "done" },
            "result_field": "result"
        }
    });
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    fs::create_dir_all(&dir).expect("create the kinds directory");
    fs::write(dir.join("fake.json"), kind.to_string()).expect("write the fake kind");
}

/// The service, with the fake agent told to report whether `PROJECT_TOKEN` is set.
fn spawn_service(sandbox: &Sandbox) -> ChildGuard {
    sandbox.track("default");
    let child = massorca_in(sandbox.home())
        .args(["--profile", "default", "service", "run"])
        .env("FAKE_AGENT_PROBE_ENV", "PROJECT_TOKEN")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the service starts");
    let guard = ChildGuard::new(child);
    sandbox.wait_for_running("default", true);
    guard
}

/// A folder project with workspace `main` and a project secret `PROJECT_TOKEN`.
fn project_with_secret(sandbox: &Sandbox) {
    let notes = sandbox.home().join("notes");
    fs::create_dir_all(&notes).expect("the project folder");
    let added = sandbox.json_ok(
        "default",
        &[
            "project",
            "add",
            notes.to_str().expect("a UTF-8 path"),
            "--folder",
        ],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    let scope = format!("project:{}", added["id"].as_str().expect("a project id"));
    sandbox
        .run_with_input(
            "default",
            &["secret", "set", &scope, "PROJECT_TOKEN"],
            b"placeholder-run-path-4e19\n",
        )
        .success();
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

fn text_of<'a>(record: &'a Value, field: &str) -> &'a str {
    record[field].as_str().expect("a text field")
}

fn wait_for_line(sandbox: &Sandbox, terminal: &str, line: &str) {
    eventually(&format!("`{line}` on the agent's screen"), || {
        let screen = sandbox.json_ok("default", &["terminal", "screen", terminal]);
        screen["lines"]
            .as_array()
            .is_some_and(|lines| {
                lines
                    .iter()
                    .any(|shown| shown.as_str().map(str::trim) == Some(line))
            })
            .then_some(())
    });
}

#[test]
fn a_resume_while_the_vault_is_locked_is_refused_and_changes_nothing() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let _service = spawn_service(&sandbox);
    project_with_secret(&sandbox);
    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            "main",
            "--prompt",
            "finish up",
        ],
    );
    let session = text_of(&started, "id").to_owned();
    until(&sandbox, &session, "done");
    sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);
    let before = sandbox.json_ok("default", &["agent", "status", &session]);

    // Without its protection record the vault is locked.
    fs::remove_file(
        sandbox
            .profile_dir("default")
            .join("vault")
            .join("protection"),
    )
    .expect("the protection record goes");
    let outcome = sandbox
        .run("default", &with_json(&["agent", "resume", &session]))
        .failure();
    let error = single_document(&outcome)["error"].clone();
    assert_eq!(error["code"], "agent.secrets_unavailable", "{error}");
    assert!(
        !error.to_string().contains("placeholder-run-path-4e19"),
        "{error}"
    );
    let after = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(after, before, "the session row is unchanged");
}

#[test]
fn a_delegated_child_receives_its_project_secret() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let _service = spawn_service(&sandbox);
    project_with_secret(&sandbox);
    let parent = sandbox.json_ok(
        "default",
        &["agent", "start", "fake", "--workspace", "main"],
    );
    let parent_id = text_of(&parent, "id").to_owned();
    until(&sandbox, &parent_id, "idle");

    let child = sandbox.json_ok(
        "default",
        &[
            "agent",
            "delegate",
            "--from",
            &parent_id,
            "--prompt",
            "look around",
            "--read-only",
        ],
    );
    wait_for_line(
        &sandbox,
        text_of(&child, "terminal_id"),
        "fake-agent: env PROJECT_TOKEN set",
    );
    assert_eq!(child["holds_secrets"], true, "the child is marked: {child}");
}
