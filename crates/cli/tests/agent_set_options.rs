//! `massorca agent set-options` (decision 0171) through the CLI, against the fake agent. The fake kind declares a model with a
//! live command, two efforts, and fast mode with a live toggle. The fake prints the model, effort and fast mode it starts
//! with, each live model it reads (`/model <id>`) and each fast toggle (`/fast`), so the tests read the agent's screen to
//! see what reached it.

mod harness;

use harness::{Sandbox, eventually, single_document, stdout_text, with_json};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// Generous upper bound for `--until`; the wait ends as soon as the state is observed.
const UNTIL_SECS: &str = "60";

fn status() -> Value {
    json!({
        "mechanism": "env-command",
        "events": { "start": "idle", "prompt": "working", "stop": "done" },
        "result_field": "result"
    })
}

/// Declares the fake kind `fake`: a model with a live command (`opus` takes fast mode), two efforts, and fast mode with a
/// live toggle.
fn declare(sandbox: &Sandbox) {
    let kind = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
        "resume": ["--resume", "{ref}"],
        "status": status(),
        "model": {
            "flag": "--model",
            "live": "/model {value}",
            "choices": [
                { "id": "opus", "label": "Opus", "fast": true },
                { "id": "haiku", "label": "Haiku" }
            ]
        },
        "effort": {
            "flag": "--effort",
            "levels": [{ "id": "low", "label": "Low" }, { "id": "high", "label": "High" }]
        },
        "fast": {
            "settings": { "fastMode": true },
            "settings_flag": "--settings",
            "live": "/fast",
            "toggle": true
        }
    });
    sandbox.prepare_profile("default");
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join("fake.json"), kind.to_string()).expect("write a kind file");
}

/// A sandbox with the fake kind and a folder workspace `main`.
fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    declare(&sandbox);
    let folder = sandbox.home().join("notes");
    std::fs::create_dir_all(&folder).expect("create folder");
    let folder_text = folder.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", folder_text.as_str(), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    sandbox
}

fn start(sandbox: &Sandbox, extra: &[&str]) -> Value {
    let mut args = vec![
        "agent",
        "start",
        "fake",
        "--workspace",
        "main",
        "--read-only",
    ];
    args.extend_from_slice(extra);
    sandbox.json_ok("default", &args)
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

/// Runs `--json <args>`, which must fail, and returns the error document.
fn error_of(sandbox: &Sandbox, args: &[&str]) -> Value {
    let outcome = sandbox.run("default", &with_json(args)).failure();
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    document["error"].clone()
}

/// The agent's screen, trimmed lines.
fn screen(sandbox: &Sandbox, terminal: &str) -> Vec<String> {
    let screen = sandbox.json_ok("default", &["terminal", "screen", terminal]);
    screen["lines"]
        .as_array()
        .map(|lines| {
            lines
                .iter()
                .filter_map(|line| line.as_str().map(|line| line.trim().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

/// Waits until the terminal's screen shows `line`; returns the screen then.
fn wait_for_line(sandbox: &Sandbox, terminal: &str, line: &str) -> Vec<String> {
    eventually(&format!("`{line}` on the agent's screen"), || {
        let shown = screen(sandbox, terminal);
        shown.iter().any(|shown| shown == line).then_some(shown)
    })
}

fn stop(sandbox: &Sandbox, session: &str) {
    sandbox.json_ok("default", &["agent", "stop", session, "--force"]);
}

/// Stops the session's run and waits until it has exited.
fn stopped(sandbox: &Sandbox, session: &str) {
    stop(sandbox, session);
    until(sandbox, session, "exited");
}

fn resume(sandbox: &Sandbox, session: &str) -> Value {
    sandbox.json_ok("default", &["agent", "resume", session])
}

/// `agent set-options <session> <extra...>`, which must succeed.
fn set_options(sandbox: &Sandbox, session: &str, extra: &[&str]) -> Value {
    let mut args = vec!["agent", "set-options", session];
    args.extend_from_slice(extra);
    sandbox.json_ok("default", &args)
}

#[test]
fn a_model_is_typed_into_the_running_agent_and_applied_live() {
    let sandbox = sandbox();
    let started = start(&sandbox, &[]);
    let session = id_of(&started);
    until(&sandbox, &session, "idle");

    let set = set_options(&sandbox, &session, &["--model", "haiku"]);
    assert_eq!(set["applied"], json!({ "model": "live" }), "{set}");
    wait_for_line(
        &sandbox,
        &terminal_of(&started),
        "fake-agent: model switched haiku",
    );

    // The run has haiku now: asking for it again types nothing and says so.
    let again = stdout_text(
        &sandbox
            .run(
                "default",
                &["agent", "set-options", &session, "--model", "haiku"],
            )
            .success(),
    );
    assert!(again.contains("nothing changed"), "{again}");

    stop(&sandbox, &session);
}

#[test]
fn a_live_change_says_it_switched_in_the_running_session() {
    let sandbox = sandbox();
    let started = start(&sandbox, &["--model", "opus"]);
    let session = id_of(&started);
    until(&sandbox, &session, "idle");

    let text = stdout_text(
        &sandbox
            .run(
                "default",
                &["agent", "set-options", &session, "--model", "haiku"],
            )
            .success(),
    );
    assert!(
        text.contains("model switched in the running session"),
        "{text}"
    );
    stop(&sandbox, &session);
}

#[test]
fn an_effort_waits_for_the_next_resume_and_the_resumed_run_takes_it() {
    let sandbox = sandbox();
    let started = start(&sandbox, &[]);
    let session = id_of(&started);
    until(&sandbox, &session, "idle");

    let set = set_options(&sandbox, &session, &["--effort", "high"]);
    assert_eq!(set["applied"], json!({ "effort": "next_resume" }), "{set}");
    let shown = screen(&sandbox, &terminal_of(&started));
    assert!(
        !shown.iter().any(|line| line.contains("effort high")),
        "nothing was typed into the running agent: {shown:?}"
    );

    stopped(&sandbox, &session);
    let resumed = resume(&sandbox, &session);
    wait_for_line(&sandbox, &terminal_of(&resumed), "fake-agent: effort high");
    stop(&sandbox, &session);
}

#[test]
fn a_stopped_session_resumes_with_the_model_and_fast_mode_it_was_given() {
    let sandbox = sandbox();
    let started = start(&sandbox, &[]);
    let session = id_of(&started);
    until(&sandbox, &session, "idle");
    stopped(&sandbox, &session);

    let set = set_options(&sandbox, &session, &["--model", "opus", "--fast"]);
    assert_eq!(
        set["applied"],
        json!({ "model": "next_resume", "fast": "next_resume" }),
        "{set}"
    );
    assert!(set["session"].get("fast").is_none(), "{set}");

    let resumed = resume(&sandbox, &session);
    wait_for_line(&sandbox, &terminal_of(&resumed), "fake-agent: model opus");
    wait_for_line(&sandbox, &terminal_of(&resumed), "fake-agent: fast mode on");
    assert_eq!(resumed["fast"], json!({ "on": true, "confirmed": false }));
    stop(&sandbox, &session);
}

#[test]
fn a_fast_toggle_is_typed_once_and_not_again_while_the_state_is_unchanged() {
    let sandbox = sandbox();
    let started = start(&sandbox, &["--model", "opus", "--fast"]);
    let session = id_of(&started);
    until(&sandbox, &session, "idle");
    wait_for_line(&sandbox, &terminal_of(&started), "fake-agent: fast mode on");

    let same = set_options(&sandbox, &session, &["--fast"]);
    assert_eq!(
        same["applied"],
        json!({}),
        "fast mode is on already: {same}"
    );

    let off = set_options(&sandbox, &session, &["--no-fast"]);
    assert_eq!(off["applied"], json!({ "fast": "live" }), "{off}");
    assert!(off["session"].get("fast").is_none(), "{off}");
    wait_for_line(
        &sandbox,
        &terminal_of(&started),
        "fake-agent: fast toggled off",
    );

    let again = set_options(&sandbox, &session, &["--no-fast"]);
    assert_eq!(again["applied"], json!({}), "{again}");
    let toggles = screen(&sandbox, &terminal_of(&started))
        .iter()
        .filter(|line| line.contains("fast toggled"))
        .count();
    assert_eq!(toggles, 1, "the toggle was typed once");
    stop(&sandbox, &session);
}

#[test]
fn a_refused_value_changes_nothing_and_says_why() {
    let sandbox = sandbox();
    let started = start(&sandbox, &["--model", "haiku"]);
    let session = id_of(&started);
    until(&sandbox, &session, "idle");

    let fast = error_of(&sandbox, &["agent", "set-options", &session, "--fast"]);
    assert_eq!(fast["code"], "agent.option_unsupported", "{fast}");

    let effort = error_of(
        &sandbox,
        &["agent", "set-options", &session, "--effort", "turbo"],
    );
    assert_eq!(effort["code"], "request.invalid_params", "{effort}");

    let shown = screen(&sandbox, &terminal_of(&started));
    assert!(
        !shown.iter().any(|line| line.contains("switched")),
        "nothing was typed: {shown:?}"
    );
    stop(&sandbox, &session);
}

#[test]
fn a_fast_toggle_the_run_started_with_is_not_typed_again_and_is_said_to_be_unchanged() {
    let sandbox = sandbox();
    let started = start(&sandbox, &["--model", "opus", "--fast"]);
    let session = id_of(&started);
    until(&sandbox, &session, "idle");

    let text = stdout_text(
        &sandbox
            .run("default", &["agent", "set-options", &session, "--fast"])
            .success(),
    );
    assert!(
        text.contains("nothing changed"),
        "the text says nothing changed: {text}"
    );
    stop(&sandbox, &session);
}

/// The settle window a typed live command holds the queue for, when no report ends it first (decision 0171).
const SETTLE_WINDOW: Duration = Duration::from_secs(2);

#[test]
fn a_queued_prompt_after_a_typed_command_is_typed_once_the_settle_window_ends() {
    let sandbox = sandbox();
    let started = start(&sandbox, &["--model", "opus"]);
    let session = id_of(&started);
    until(&sandbox, &session, "idle");

    // The command types, then a prompt is queued at once. No report follows a typed command, so only the window's end
    // may release the queue.
    let since = Instant::now();
    let set = set_options(&sandbox, &session, &["--model", "haiku"]);
    assert_eq!(set["applied"], json!({ "model": "live" }), "{set}");
    sandbox.json_ok(
        "default",
        &["agent", "queue", "add", &session, "after the switch"],
    );
    wait_for_line(
        &sandbox,
        &terminal_of(&started),
        "fake-agent: done after the switch",
    );
    // The prompt was typed after the window, so its answer comes no sooner than the window after the command.
    assert!(
        since.elapsed() >= SETTLE_WINDOW,
        "typed inside the settle window: {:?}",
        since.elapsed()
    );
    stop(&sandbox, &session);
}
