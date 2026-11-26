//! Fast mode through the CLI (decision 0165). The fake kind takes a model `big` that offers fast mode and a model `small`
//! that does not; `massorca-fake-agent` prints `fast mode on` when it is started with a `--settings` object that turns
//! `fastMode` on. A second kind takes no fast mode at all.

mod harness;

use harness::{Sandbox, eventually, single_document, stdout_text, with_json};
use serde_json::{Value, json};

fn status() -> Value {
    json!({
        "mechanism": "env-command",
        "events": { "start": "idle", "prompt": "working", "stop": "done" },
        "result_field": "result"
    })
}

/// Writes the profile's kind `id` with the given extra fields.
fn declare(sandbox: &Sandbox, id: &str, extra: Value) {
    let mut kind = json!({
        "id": id,
        "name": format!("Fake agent {id}"),
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "status": status(),
    });
    for (key, value) in extra.as_object().expect("an object") {
        kind[key] = value.clone();
    }
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join(format!("{id}.json")), kind.to_string()).expect("write a kind file");
}

/// A sandbox with a folder workspace `main`, the kind `fake-fast` (fast mode on `big` only) and the kind `fake-plain`.
fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    declare(
        &sandbox,
        "fake-fast",
        json!({
            "model": { "flag": "--model", "choices": [
                { "id": "big", "label": "Big", "fast": true },
                { "id": "small", "label": "Small" }
            ] },
            "fast": { "settings": { "fastMode": true } }
        }),
    );
    declare(&sandbox, "fake-plain", json!({}));
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

#[test]
fn fast_passes_the_settings_flag_and_the_session_shows_it_requested() {
    let sandbox = sandbox();
    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake-fast",
            "--workspace",
            "main",
            "--read-only",
            "--model",
            "big",
            "--fast",
        ],
    );
    assert_eq!(
        started["fast"],
        json!({ "on": true, "confirmed": false }),
        "{started}"
    );
    let session = started["id"].as_str().expect("session id").to_owned();
    let terminal = started["terminal_id"]
        .as_str()
        .expect("terminal id")
        .to_owned();

    wait_for_line(&sandbox, &terminal, "fake-agent: fast mode on");
    let text = stdout_text(
        &sandbox
            .run("default", &["agent", "status", &session])
            .success(),
    );
    assert!(text.lines().any(|line| line == "fast: requested"), "{text}");
    stop(&sandbox, &session);
}

#[test]
fn a_model_without_fast_mode_is_refused_and_named() {
    let sandbox = sandbox();
    let error = error_of(
        &sandbox,
        &[
            "agent",
            "start",
            "fake-fast",
            "--workspace",
            "main",
            "--read-only",
            "--model",
            "small",
            "--fast",
        ],
    );
    assert_eq!(error["code"], "agent.option_unsupported", "{error}");
    let message = error["message"].as_str().unwrap_or_default();
    assert!(message.contains("small"), "{error}");
    assert!(message.contains("fake-fast"), "{error}");
}

#[test]
fn a_kind_without_fast_mode_is_refused_and_named() {
    let sandbox = sandbox();
    let error = error_of(
        &sandbox,
        &[
            "agent",
            "start",
            "fake-plain",
            "--workspace",
            "main",
            "--read-only",
            "--fast",
        ],
    );
    assert_eq!(error["code"], "agent.option_unsupported", "{error}");
    assert!(
        error["message"]
            .as_str()
            .unwrap_or_default()
            .contains("fake-plain"),
        "{error}"
    );
}

#[test]
fn a_start_without_fast_passes_nothing_and_shows_no_fast_line() {
    let sandbox = sandbox();
    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake-fast",
            "--workspace",
            "main",
            "--read-only",
            "--model",
            "big",
        ],
    );
    assert!(started.get("fast").is_none(), "{started}");
    let session = started["id"].as_str().expect("session id").to_owned();
    let terminal = started["terminal_id"]
        .as_str()
        .expect("terminal id")
        .to_owned();

    wait_for_line(&sandbox, &terminal, "fake-agent: model big");
    assert!(
        !screen(&sandbox, &terminal)
            .iter()
            .any(|line| line == "fake-agent: fast mode on"),
        "no fast settings reached the agent"
    );
    let text = stdout_text(
        &sandbox
            .run("default", &["agent", "status", &session])
            .success(),
    );
    assert!(
        !text.lines().any(|line| line.starts_with("fast:")),
        "{text}"
    );
    stop(&sandbox, &session);
}
