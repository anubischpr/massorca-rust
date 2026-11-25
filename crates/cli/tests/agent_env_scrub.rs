//! What a run inherits from the service (decision 0160): MassOrca's own run identity and the markers a kind lists in
//! `strip_inherited_env` never reach it, whatever the process that started the service had set. The values here are
//! fake; the fake agent prints only whether an inherited value reached it, never a value.

mod harness;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use harness::{ChildGuard, Sandbox, eventually, massorca_in};
use serde_json::json;

/// The identity of a session the service was started from, as a delegated parent's environment would carry it.
const PARENT_SESSION: &str = "fake-parent-session";
const PARENT_TOKEN: &str = "fake-parent-token";
/// A marker the fake kind strips, like the vendor stamp of an agent the service was started from.
const MARKER: &str = "CLAUDE_CODE_CHILD_SESSION";
const MARKER_VALUE: &str = "1";

/// Declares kind `fake`: the fake agent, reporting through `massorca agent hook`, stripping [`MARKER`].
fn declare_fake(sandbox: &Sandbox) {
    let dir = sandbox.profile_dir("default").join("agents").join("kinds");
    sandbox.prepare_profile("default");
    fs::create_dir_all(&dir).expect("create the kinds directory");
    let kind = json!({
        "id": "fake",
        "name": "Fake agent",
        "program": [env!("CARGO_BIN_EXE_massorca-fake-agent")],
        "prompt": { "mode": "argv" },
        "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
        "resume": ["--resume", "{ref}"],
        "status": { "mechanism": "env-command", "events": { "start": "idle", "stop": "done" } },
        "strip_inherited_env": [MARKER]
    });
    fs::write(dir.join("fake.json"), kind.to_string()).expect("write the fake kind");
}

/// Starts the service as if from inside another agent run: with that run's identity and the vendor marker set. On
/// Windows the marker is spelled in lower case, since names compare without case there.
fn spawn_service_inside_a_run(sandbox: &Sandbox, user_home: &Path) -> ChildGuard {
    sandbox.track("default");
    let marker = if cfg!(windows) {
        MARKER.to_ascii_lowercase()
    } else {
        MARKER.to_owned()
    };
    let foreign = format!(
        "MASSORCA_AGENT_SESSION={PARENT_SESSION},MASSORCA_AGENT_TOKEN={PARENT_TOKEN},{MARKER}={MARKER_VALUE}"
    );
    let mut command = massorca_in(sandbox.home());
    command
        .args(["--profile", "default", "service", "run"])
        .env("HOME", user_home)
        .env("USERPROFILE", user_home)
        .env("MASSORCA_AGENT_SESSION", PARENT_SESSION)
        .env("MASSORCA_AGENT_TOKEN", PARENT_TOKEN)
        .env(&marker, MARKER_VALUE)
        .env("FAKE_AGENT_PROBE_FOREIGN", foreign)
        .env("FAKE_AGENT_PROBE_ENV", MARKER)
        .env_remove("FAKE_AGENT_HOME")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for name in ["CLAUDE_CONFIG_DIR", "CODEX_HOME", "PI_CODING_AGENT_DIR"] {
        command.env_remove(name);
    }
    let guard = ChildGuard::new(command.spawn().expect("the service starts"));
    sandbox.wait_for_running("default", true);
    guard
}

/// A fake user home for the service, so nothing reads the real one. Windows resolves the service's own known folders
/// below `USERPROFILE`, and refuses them when they are absent.
fn fake_user_home(sandbox: &Sandbox) -> PathBuf {
    let home = sandbox.home().join("user");
    for known in [["AppData", "Roaming"], ["AppData", "Local"]] {
        fs::create_dir_all(home.join(known[0]).join(known[1])).expect("create a known folder");
    }
    home
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

/// Waits until the terminal's screen shows `line`.
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
fn a_run_never_sees_an_inherited_run_identity_or_a_stripped_marker() {
    let sandbox = Sandbox::new();
    declare_fake(&sandbox);
    let user_home = fake_user_home(&sandbox);
    let _service = spawn_service_inside_a_run(&sandbox, &user_home);
    let notes = sandbox.home().join("notes");
    fs::create_dir_all(&notes).unwrap();
    sandbox.json_ok(
        "default",
        &["project", "add", path_text(&notes), "--folder"],
    );
    sandbox.json_ok("default", &["workspace", "create", "notes", "main"]);

    let started = sandbox.json_ok(
        "default",
        &["agent", "start", "fake", "--workspace", "main"],
    );
    let session = started["id"].as_str().expect("a session id").to_owned();
    let terminal = started["terminal_id"]
        .as_str()
        .expect("a terminal id")
        .to_owned();
    wait_for_line(
        &sandbox,
        &terminal,
        "fake-agent: env MASSORCA_AGENT_SESSION own",
    );
    wait_for_line(
        &sandbox,
        &terminal,
        "fake-agent: env MASSORCA_AGENT_TOKEN own",
    );
    wait_for_line(
        &sandbox,
        &terminal,
        &format!("fake-agent: env {MARKER} own"),
    );
    // The terminal can only add variables, so the marker is emptied rather than removed (docs/architecture/agents.md).
    wait_for_line(
        &sandbox,
        &terminal,
        &format!("fake-agent: env {MARKER} empty"),
    );

    // The run reports with its own session and token: the hook reaches this session.
    eventually("the run's own hook report", || {
        let shown = sandbox.json_ok("default", &["agent", "status", &session]);
        (shown["activity"]["state"] == "idle").then_some(())
    });

    // A plain terminal of the same service carries no inherited run identity either.
    let plain = sandbox.json_ok(
        "default",
        &[
            "terminal",
            "create",
            "main",
            "--shell",
            env!("CARGO_BIN_EXE_massorca-fake-agent"),
        ],
    );
    let plain = plain["id"].as_str().expect("a terminal id").to_owned();
    wait_for_line(
        &sandbox,
        &plain,
        "fake-agent: env MASSORCA_AGENT_SESSION own",
    );
    wait_for_line(&sandbox, &plain, "fake-agent: env MASSORCA_AGENT_TOKEN own");
}
