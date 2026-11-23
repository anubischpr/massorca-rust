//! A service that dies between starting an agent's program and recording its session (docs/architecture/agents.md,
//! "Crash-safe start"). The service is run with `MASSORCA_TEST_CRASH_AT`, which makes it exit at once at that point, as a
//! crash would; the next service start must settle what it left.

mod harness;

use std::path::PathBuf;
use std::process::Stdio;

use harness::{ChildGuard, Sandbox, massorca_in};
use serde_json::{Value, json};

const CRASH_AT_ENV: &str = "MASSORCA_TEST_CRASH_AT";
/// The exit code of a test crash point.
const CRASH_EXIT: i32 = 70;

/// A sandbox with kind `fake` (resumable, with a status mechanism) and a folder workspace `main`.
fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
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
    sandbox.prepare_profile("default");
    std::fs::create_dir_all(&dir).expect("create the kinds directory");
    std::fs::write(dir.join("fake.json"), kind.to_string()).expect("write the kind file");
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

fn runs_dir(sandbox: &Sandbox) -> PathBuf {
    sandbox.profile_dir("default").join("agents").join("runs")
}

/// The run directories that hold a start mark.
fn marked_runs(sandbox: &Sandbox) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(runs_dir(sandbox)) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.path().join("starting").is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

fn run_dirs(sandbox: &Sandbox) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(runs_dir(sandbox)) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

/// Stops the running service, then runs one that exits at `point`, and waits until it serves.
fn crashing_service(sandbox: &Sandbox, point: &str) -> ChildGuard {
    sandbox.run("default", &["service", "stop"]).success();
    sandbox.wait_for_running("default", false);
    let mut command = massorca_in(sandbox.home());
    let child = command
        .args(["--profile", "default", "service", "run"])
        .env(CRASH_AT_ENV, point)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the service process starts");
    let guard = ChildGuard::new(child);
    sandbox.wait_for_running("default", true);
    guard
}

/// Runs `args` against the crashing service, which must not answer, and waits for the service to exit at its crash point.
fn crash_on(sandbox: &Sandbox, service: &mut ChildGuard, args: &[&str]) {
    let mut all = vec!["--no-autostart", "--json"];
    all.extend_from_slice(args);
    sandbox.run("default", &all).failure();
    let status = service.wait_for_exit();
    assert_eq!(
        status.code(),
        Some(CRASH_EXIT),
        "the service exits at its crash point"
    );
}

fn sessions(sandbox: &Sandbox) -> Vec<Value> {
    sandbox.json_ok("default", &["agent", "list"])["sessions"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// A crash after the program started and before the session row existed leaves a marked run directory and no session;
/// the next service start deletes the directory, and a new start works.
#[test]
fn a_start_that_crashed_before_its_session_was_recorded_is_cleaned_up_at_the_next_start() {
    let sandbox = sandbox();
    let mut service = crashing_service(&sandbox, "agent-start-spawned");
    crash_on(
        &sandbox,
        &mut service,
        &[
            "agent",
            "start",
            "fake",
            "--workspace",
            "main",
            "--read-only",
        ],
    );
    let marked = marked_runs(&sandbox);
    assert_eq!(
        marked.len(),
        1,
        "the interrupted start left its mark: {marked:?}"
    );

    // The next command starts a service, which settles the interrupted start before it serves.
    assert!(
        sessions(&sandbox).is_empty(),
        "the interrupted start has no session"
    );
    assert!(
        run_dirs(&sandbox).is_empty(),
        "the interrupted start's run directory is deleted"
    );

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
    let listed = sessions(&sandbox);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0]["id"], started["id"]);
    assert!(
        marked_runs(&sandbox).is_empty(),
        "a finished start leaves no mark"
    );
}

/// A crash after a resumed program started and before the row named its terminal keeps the session: the next service
/// start removes only the mark, and the session resumes again.
#[test]
fn a_resume_that_crashed_before_its_run_was_recorded_keeps_the_session() {
    let sandbox = sandbox();
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
    let session = started["id"].as_str().expect("session id").to_owned();
    assert!(
        marked_runs(&sandbox).is_empty(),
        "a finished start leaves no mark"
    );
    let stopped = sandbox.json_ok("default", &["agent", "stop", &session, "--force"]);
    assert_eq!(stopped["execution"]["state"], "exited");

    let mut service = crashing_service(&sandbox, "agent-resume-spawned");
    crash_on(&sandbox, &mut service, &["agent", "resume", &session]);
    assert_eq!(marked_runs(&sandbox), vec![session.clone()]);

    let listed = sessions(&sandbox);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0]["id"], session.as_str());
    assert_eq!(listed[0]["execution"]["state"], "exited");
    assert!(marked_runs(&sandbox).is_empty(), "the mark is removed");

    let resumed = sandbox.json_ok("default", &["agent", "resume", &session]);
    assert_eq!(resumed["id"], session.as_str());
    assert_eq!(resumed["execution"]["state"], "live");
    assert!(
        marked_runs(&sandbox).is_empty(),
        "a finished resume leaves no mark"
    );
}
