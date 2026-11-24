//! `terminal wait` through the CLI: the exit code of a script, a terminal that already exited, a wait that runs out, a
//! terminal closed during the wait, a condition this build does not know, and the help text.

mod harness;

use std::fs::File;
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

use harness::{ChildGuard, Sandbox, single_document, with_json};
use predicates::str::contains;
use serde_json::Value;

/// Registers `<home>/notes` as a folder project and creates its workspace `main`.
fn main_workspace(sandbox: &Sandbox) {
    let notes = sandbox.home().join("notes");
    std::fs::create_dir_all(&notes).expect("create folder");
    let notes_text = notes.to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &["project", "add", notes_text.as_str(), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);
}

/// Starts a terminal in `main` that runs `script` and exits; returns its id.
fn script_terminal(sandbox: &Sandbox, script: &str) -> String {
    let mut args = vec!["terminal", "create", "main"];
    if cfg!(windows) {
        args.extend(["--shell", "cmd.exe", "--arg", "/d", "--arg", "/c"]);
    } else {
        args.extend(["--shell", "sh", "--arg", "-c"]);
    }
    args.extend(["--arg", script]);
    let created = sandbox.json_ok("default", &args);
    created["id"].as_str().expect("the terminal id").to_owned()
}

/// Starts a terminal in `main` with the default shell, which stays live; returns its id.
fn live_terminal(sandbox: &Sandbox) -> String {
    let created = sandbox.json_ok("default", &["terminal", "create", "main"]);
    created["id"].as_str().expect("the terminal id").to_owned()
}

#[test]
fn a_wait_reports_the_exit_code_of_a_script_that_ends() {
    let sandbox = Sandbox::new();
    main_workspace(&sandbox);
    let id = script_terminal(&sandbox, "exit 3");

    let waited = sandbox.json_ok("default", &["terminal", "wait", &id, "--timeout", "30"]);
    assert_eq!(waited["satisfied"], true, "{waited}");
    assert_eq!(waited["status"], "exited");
    assert_eq!(waited["exit"]["code"], 3);
}

#[test]
fn a_wait_on_a_terminal_that_already_exited_prints_its_code_at_once() {
    let sandbox = Sandbox::new();
    main_workspace(&sandbox);
    let id = script_terminal(&sandbox, "exit 3");
    sandbox.json_ok("default", &["terminal", "wait", &id, "--timeout", "30"]);

    let started = Instant::now();
    sandbox
        .run("default", &["terminal", "wait", &id, "--timeout", "0"])
        .success()
        .stdout(contains("exit: exited, code 3"));
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "an exited terminal does not wait: {:?}",
        started.elapsed()
    );
}

#[test]
fn a_wait_that_runs_out_fails_with_a_stable_code_and_the_live_state() {
    let sandbox = Sandbox::new();
    main_workspace(&sandbox);
    let id = live_terminal(&sandbox);

    let outcome = sandbox
        .run(
            "default",
            &with_json(&["terminal", "wait", &id, "--timeout", "1"]),
        )
        .code(1);
    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    assert_eq!(document["error"]["code"], "terminal.wait_timeout");
    assert_eq!(document["error"]["data"]["status"], "live");
}

#[test]
fn a_terminal_closed_during_a_wait_fails_as_gone() {
    let sandbox = Sandbox::new();
    main_workspace(&sandbox);
    let id = live_terminal(&sandbox);

    let out_path = sandbox.home().join("waiter.json");
    let child = sandbox
        .command("default")
        .args(with_json(&["terminal", "wait", &id, "--timeout", "60"]))
        .stdout(Stdio::from(
            File::create(&out_path).expect("the output file"),
        ))
        .stderr(Stdio::null())
        .spawn()
        .expect("the wait starts");
    let mut waiter = ChildGuard::new(child);
    // The wait must reach the service before the terminal is closed.
    thread::sleep(Duration::from_secs(2));
    sandbox
        .run("default", &["terminal", "close", &id])
        .success();

    let status = waiter.wait_for_exit();
    assert_eq!(status.code(), Some(1));
    let text = std::fs::read_to_string(&out_path).expect("the wait's output");
    let document: Value = serde_json::from_str(&text).expect("one JSON document");
    assert_eq!(document["error"]["code"], "terminal.gone", "{document}");
}

#[test]
fn a_condition_this_build_does_not_know_is_a_usage_error() {
    let sandbox = Sandbox::new();
    sandbox
        .run(
            "default",
            &[
                "terminal",
                "wait",
                "t-1",
                "--for",
                "tui-idle",
                "--no-autostart",
            ],
        )
        .code(2);
}

#[test]
fn the_wait_command_documents_its_condition_and_timeout() {
    let sandbox = Sandbox::new();
    sandbox
        .run("default", &["terminal", "wait", "--help"])
        .success()
        .stdout(contains("--for"))
        .stdout(contains("--timeout"));
}
