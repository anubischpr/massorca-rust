//! Agent status through the CLI (decisions 0052, 0053): the fake agent reports through `massorca agent hook`, and
//! the tests wait on reported states, never on fixed sleeps.

mod harness;

use std::time::{Duration, Instant};

use harness::{Sandbox, capture, eventually};
use serde_json::{Value, json};

/// Declares kind `fake` with the full event table, or with `status` replaced by `status_override`.
fn declare_fake(sandbox: &Sandbox, status_override: Option<Value>) {
    let status = status_override.unwrap_or_else(|| {
        json!({
            "mechanism": "env-command",
            "events": {
                "start": "idle", "prompt": "working", "ask": "waiting",
                "stop": "done", "fail": "done:failure"
            },
            "result_field": "result"
        })
    });
    let kind = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
        "status": status
    });
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join("fake.json"), kind.to_string()).expect("write the kind file");
}

/// A sandbox with kind `fake` and a folder workspace `main`.
fn sandbox_with_workspace(status_override: Option<Value>) -> Sandbox {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox, status_override);
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    sandbox
}

/// Starts the fake agent with `prompt`; returns the session id and its terminal id.
fn start(sandbox: &Sandbox, prompt: &str, extra: &[&str]) -> (String, String) {
    let mut args = vec![
        "agent",
        "start",
        "fake",
        "--workspace",
        "main",
        "--prompt",
        prompt,
    ];
    args.extend_from_slice(extra);
    let started = sandbox.json_ok("default", &args);
    (
        started["id"].as_str().expect("session id").to_owned(),
        started["terminal_id"]
            .as_str()
            .expect("terminal id")
            .to_owned(),
    )
}

fn activity(session: &Value) -> (&str, Option<&str>, &str) {
    let activity = &session["activity"];
    (
        activity["state"].as_str().unwrap_or_default(),
        activity["outcome"].as_str(),
        activity["source"].as_str().unwrap_or_default(),
    )
}

#[test]
fn a_report_with_a_wrong_token_is_refused_and_the_hook_still_exits_zero() {
    let sandbox = sandbox_with_workspace(None);
    let (session, _terminal) = start(&sandbox, "hold on", &[]);
    eventually("the agent working", || {
        let shown = sandbox.json_ok("default", &["agent", "status", &session]);
        (shown["activity"]["state"] == "working").then_some(())
    });

    let mut hook = sandbox.command("default");
    hook.args(["agent", "hook", "stop", r#"{"result":"forged"}"#])
        .env("MASSORCA_AGENT_SESSION", &session)
        .env("MASSORCA_AGENT_TOKEN", "not-the-token");
    let outcome = harness::execute(&mut hook).success();
    let output = outcome.get_output();
    assert!(output.stdout.is_empty(), "a hook prints nothing on stdout");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("agent.hook_refused"), "{stderr}");

    let shown = sandbox.json_ok("default", &["agent", "status", &session]);
    assert_eq!(
        activity(&shown),
        ("working", None, "hook"),
        "a refused report changes nothing"
    );
    assert!(shown.get("result").is_none(), "{shown}");
}

#[test]
fn a_hook_outside_an_agent_run_exits_zero_promptly() {
    let sandbox = Sandbox::new();
    let mut hook = sandbox.command("default");
    hook.args(["agent", "hook", "start"])
        .env_remove("MASSORCA_AGENT_SESSION")
        .env_remove("MASSORCA_AGENT_TOKEN");
    let began = Instant::now();
    let output = capture(&mut hook, Duration::from_secs(30)).expect("the hook finishes in time");
    assert!(output.status.success(), "{output:?}");
    assert!(
        began.elapsed() < Duration::from_secs(30),
        "within the bound"
    );
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("MASSORCA_AGENT_SESSION"), "{stderr}");
    let status = sandbox.json_ok("default", &["service", "status"]);
    assert_eq!(
        status["running"], false,
        "a hook never starts the service: {status}"
    );
}
