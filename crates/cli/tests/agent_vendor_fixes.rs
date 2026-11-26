//! Vendor CLI fixes (decision 0164). The Manual mode of Claude Code is declared as `--permission-mode {value}` with the
//! launch values `default` then `manual`: a run gets the first spelling the CLI's `--help` lists for the flag, and
//! `default` when the help lists neither or cannot be read. The fake agent prints the choices its help lists
//! (`FAKE_AGENT_HELP_PERMISSION_CHOICES`), or fails its help (`FAKE_AGENT_HELP_FAILS=1`), and prints the mode it was
//! started in, so each test reads that mode from the agent's screen.

mod harness;

use std::fs;

use harness::{Sandbox, eventually};
use serde_json::json;

/// Declares kind `fake-claude`: the fake agent with a Manual mode whose launch value is chosen from the help, and a plain
/// `plan` mode whose launch argument is its own.
fn declare(sandbox: &Sandbox) {
    let kind = json!({
        "id": "fake-claude",
        "name": "Fake Claude",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
        "resume": ["--resume", "{ref}"],
        "status": {
            "mechanism": "env-command",
            "events": { "start": "idle", "prompt": "working", "stop": "done" },
            "result_field": "result"
        },
        "permission_modes": [
            {
                "id": "default",
                "label": "Manual",
                "args": ["--permission-mode", "{value}"],
                "launch_values": ["default", "manual"]
            },
            { "id": "plan", "label": "Plan", "args": ["--permission-mode", "plan"] }
        ],
        "default_permission_mode": "plan"
    });
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    fs::create_dir_all(&dir).expect("create the kinds directory");
    fs::write(dir.join("fake-claude.json"), kind.to_string()).expect("write the kind");
}

/// A sandbox with the kind and a folder workspace `main`. The fake agent's help lists `choices` for `--permission-mode`
/// (unless it is `None`), and fails when `help_fails` is set; the service and every run inherit both.
fn sandbox(choices: Option<&str>, help_fails: bool) -> Sandbox {
    let sandbox = Sandbox::new();
    if let Some(choices) = choices {
        sandbox.set_env("FAKE_AGENT_HELP_PERMISSION_CHOICES", choices);
    }
    if help_fails {
        sandbox.set_env("FAKE_AGENT_HELP_FAILS", "1");
    }
    declare(&sandbox);
    let notes = sandbox.home().join("notes");
    fs::create_dir_all(&notes).expect("create folder");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
    sandbox
}

/// The mode the fake agent says it was started in: its `mode <id>` line, once the agent's screen shows it.
fn started_mode(sandbox: &Sandbox, terminal: &str) -> String {
    eventually("the agent's start line", || {
        let screen = sandbox.json_ok("default", &["terminal", "screen", terminal]);
        screen["lines"]
            .as_array()?
            .iter()
            .filter_map(|line| line.as_str())
            .map(str::trim)
            .find_map(|line| line.strip_prefix("fake-agent: mode ").map(str::to_owned))
    })
}

/// Starts the fake Claude kind in permission mode `mode`, reads the mode the agent was started in, and stops the session.
fn launched_mode(sandbox: &Sandbox, mode: &str) -> String {
    let started = sandbox.json_ok(
        "default",
        &[
            "agent",
            "start",
            "fake-claude",
            "--workspace",
            "main",
            "--read-only",
            "--mode",
            mode,
        ],
    );
    let session = started["id"].as_str().expect("session id").to_owned();
    let terminal = started["terminal_id"]
        .as_str()
        .expect("terminal id")
        .to_owned();
    let shown = started_mode(sandbox, &terminal);
    sandbox.json_ok("default", &["agent", "stop", session.as_str(), "--force"]);
    shown
}

#[test]
fn manual_launches_with_the_value_the_cli_help_lists_when_only_manual_is_listed() {
    let sandbox = sandbox(Some("acceptEdits,manual,plan"), false);
    assert_eq!(launched_mode(&sandbox, "default"), "manual");
}

#[test]
fn manual_keeps_default_when_the_help_lists_both_spellings() {
    let sandbox = sandbox(Some("acceptEdits,default,manual,plan"), false);
    assert_eq!(launched_mode(&sandbox, "default"), "default");
}

#[test]
fn manual_keeps_default_when_the_help_lists_neither_spelling() {
    let sandbox = sandbox(Some("acceptEdits,plan"), false);
    assert_eq!(launched_mode(&sandbox, "default"), "default");
}

#[test]
fn manual_keeps_default_when_the_help_cannot_be_read() {
    let sandbox = sandbox(Some("manual"), true);
    assert_eq!(launched_mode(&sandbox, "default"), "default");
}

#[test]
fn a_mode_without_launch_values_still_passes_its_own_argument() {
    let sandbox = sandbox(Some("manual"), false);
    assert_eq!(launched_mode(&sandbox, "plan"), "plan");
}
