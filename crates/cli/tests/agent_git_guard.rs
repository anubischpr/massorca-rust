//! An agent run's terminal runs git with prompts switched off, and a user terminal keeps the environment it inherits
//! (TERM-24). The fake agent prints the values of the variables the test lists in `FAKE_AGENT_SHOW_ENV`, all of them
//! non-secret.

mod harness;

use std::fs;
use std::path::Path;
use std::process::Stdio;

use harness::{ChildGuard, Sandbox, eventually, massorca_in};
use predicates::str::contains;
use serde_json::json;

/// What the service inherits, so every terminal too: git prompts on and no askpass helper.
const INHERITED: [(&str, &str); 2] = [("GIT_TERMINAL_PROMPT", "1"), ("GCM_INTERACTIVE", "always")];
const SHOWN: &str = "GIT_TERMINAL_PROMPT,GCM_INTERACTIVE,GIT_ASKPASS";

fn spawn_service(sandbox: &Sandbox) -> ChildGuard {
    sandbox.prepare_profile("default");
    sandbox.track("default");
    let child = massorca_in(sandbox.home())
        .args(["--profile", "default", "service", "run"])
        .envs(INHERITED)
        .env("FAKE_AGENT_SHOW_ENV", SHOWN)
        .env_remove("GIT_ASKPASS")
        .env_remove("SSH_ASKPASS")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the service starts");
    let guard = ChildGuard::new(child);
    sandbox.wait_for_running("default", true);
    guard
}

fn declare_fake(sandbox: &Sandbox) {
    sandbox.prepare_profile("default");
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    fs::create_dir_all(&dir).expect("create the kinds directory");
    let fake = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" }
    });
    fs::write(dir.join("fake.json"), fake.to_string()).expect("write the fake kind");
}

fn project_with_workspace(sandbox: &Sandbox) {
    let notes = sandbox.home().join("notes");
    fs::create_dir_all(&notes).expect("the project folder");
    sandbox.json_ok(
        "default",
        &["project", "add", path_text(&notes), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

fn wait_for_line(sandbox: &Sandbox, terminal: &str, line: &str) {
    eventually(&format!("`{line}` on the terminal's screen"), || {
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
fn an_agent_run_gets_the_git_guard_while_a_user_terminal_keeps_its_environment() {
    let sandbox = Sandbox::new();
    let _service = spawn_service(&sandbox);
    declare_fake(&sandbox);
    project_with_workspace(&sandbox);

    let started = sandbox.json_ok(
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
    let agent = started["terminal_id"]
        .as_str()
        .expect("a terminal id")
        .to_owned();
    wait_for_line(
        &sandbox,
        &agent,
        "fake-agent: value GIT_TERMINAL_PROMPT [0]",
    );
    wait_for_line(
        &sandbox,
        &agent,
        "fake-agent: value GCM_INTERACTIVE [never]",
    );
    wait_for_line(&sandbox, &agent, "fake-agent: value GIT_ASKPASS []");
    let agent_terminal = sandbox.json_ok("default", &["terminal", "show", &agent]);
    assert_eq!(agent_terminal["git_credential_guard"], true);
    sandbox
        .run("default", &["terminal", "show", &agent])
        .success()
        .stdout(contains("git credential guard: on"));

    let user = sandbox.json_ok(
        "default",
        &[
            "terminal",
            "create",
            "main",
            "--shell",
            env!("CARGO_BIN_EXE_massorca-fake-agent"),
        ],
    );
    let user = user["id"].as_str().expect("a terminal id").to_owned();
    wait_for_line(&sandbox, &user, "fake-agent: value GIT_TERMINAL_PROMPT [1]");
    wait_for_line(
        &sandbox,
        &user,
        "fake-agent: value GCM_INTERACTIVE [always]",
    );
    wait_for_line(&sandbox, &user, "fake-agent: value GIT_ASKPASS unset");
    let user_terminal = sandbox.json_ok("default", &["terminal", "show", &user]);
    assert!(
        user_terminal.get("git_credential_guard").is_none(),
        "a user terminal carries no guard"
    );
}
