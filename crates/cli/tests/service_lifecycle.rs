//! The service lifecycle through the CLI: status, info (autostart), stop and restart.

mod harness;

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::process::Stdio;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use harness::{Sandbox, execute, massorca_in, single_document};
use predicates::str::contains;
use serde_json::Value;

#[test]
fn status_reports_not_running_without_starting_anything() {
    let sandbox = Sandbox::new();

    sandbox
        .run("default", &["service", "status"])
        .success()
        .stdout(contains("not running"));

    let status = sandbox.status("default");
    assert_eq!(status["running"], false);
    assert!(status["info"].is_null());
}

#[test]
fn info_starts_the_service_and_status_then_reports_it() {
    let sandbox = Sandbox::new();

    let info = sandbox.json_ok("default", &["service", "info"]);

    assert_eq!(info["profile"], "default");
    assert!(info["pid"].as_u64().is_some_and(|pid| pid > 0));
    assert_eq!(info["protocol"], 1);
    assert!(
        info["capabilities"]
            .as_array()
            .is_some_and(|caps| caps.iter().any(|cap| cap == "service/1"))
    );

    let status = sandbox.wait_for_running("default", true);
    assert_eq!(status["info"]["pid"], info["pid"]);
    sandbox
        .run("default", &["service", "status"])
        .success()
        .stdout(contains("pid:"));
}

#[test]
fn stop_shuts_the_service_down_and_stopping_again_succeeds() {
    let sandbox = Sandbox::new();
    sandbox.json_ok("default", &["service", "info"]);

    let stopped = sandbox.json_ok("default", &["service", "stop"]);
    assert_eq!(stopped["stopped"], true);
    assert_eq!(sandbox.status("default")["running"], false);

    let again = sandbox.json_ok("default", &["service", "stop"]);
    assert_eq!(again["stopped"], false);
}

#[test]
fn info_without_autostart_exits_4_when_nothing_runs() {
    let sandbox = Sandbox::new();

    sandbox
        .run("default", &["--no-autostart", "service", "info"])
        .code(4)
        .stderr(contains("service.unavailable"));

    let outcome = sandbox
        .run("default", &["--json", "--no-autostart", "service", "info"])
        .code(4);
    assert_eq!(
        single_document(&outcome)["error"]["code"],
        "service.unavailable"
    );
    assert_eq!(sandbox.status("default")["running"], false);
}

#[test]
fn no_autostart_environment_variable_has_the_same_effect() {
    let sandbox = Sandbox::new();
    sandbox.track("default");

    execute(
        massorca_in(sandbox.home())
            .env("MASSORCA_NO_AUTOSTART", "1")
            .args(["service", "info"]),
    )
    .code(4);
}

#[test]
fn restart_starts_a_stopped_service_and_replaces_a_running_one() {
    let sandbox = Sandbox::new();

    let first = sandbox.json_ok("default", &["service", "restart"]);
    let second = sandbox.json_ok("default", &["service", "restart"]);

    assert_ne!(first["pid"], second["pid"]);
    let status = sandbox.wait_for_running("default", true);
    assert_eq!(status["info"]["pid"], second["pid"]);
}

#[test]
fn foreground_service_exits_0_when_it_is_stopped_through_the_cli() {
    let sandbox = Sandbox::new();
    let mut service = sandbox.spawn_service("default");
    sandbox.wait_for_running("default", true);

    sandbox
        .run("default", &["service", "stop"])
        .success()
        .stdout(contains("service stopped"));

    assert_eq!(service.wait_for_exit().code(), Some(0));
    assert_eq!(sandbox.status("default")["running"], false);
}

#[test]
fn log_filter_environment_variable_controls_the_service_log() {
    let sandbox = Sandbox::new();
    sandbox.track("default");
    execute(
        massorca_in(sandbox.home())
            .env("MASSORCA_LOG", "warn")
            .args(["--profile", "default", "service", "info"]),
    )
    .success();
    sandbox.wait_for_running("default", true);
    sandbox.run("default", &["service", "stop"]).success();

    let files: Vec<_> = fs::read_dir(sandbox.profile_dir("default").join("logs"))
        .expect("the logs directory exists")
        .filter_map(Result::ok)
        .collect();
    assert!(!files.is_empty(), "the service created its log file");
    let logs: String = files
        .iter()
        .map(|entry| fs::read_to_string(entry.path()).unwrap_or_default())
        .collect();
    assert!(
        !logs.contains("service started"),
        "info lines leaked: {logs:?}"
    );
}

/// Regression: the autostarted service must not inherit the caller's stdout pipe (`crates/client/src/spawn.rs`).
/// `massorca service info` exits at once, and a caller reading stdout to EOF (a shell `$(...)`, `Command::output`)
/// must not keep waiting until the service stops.
#[test]
fn autostart_does_not_keep_the_callers_stdout_pipe_open() {
    let sandbox = Sandbox::new();
    sandbox.track("default");
    let mut child = massorca_in(sandbox.home())
        .args(["--json", "service", "info"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("massorca starts");
    let mut stdout = child.stdout.take().expect("piped stdout");
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        let _ = sender.send(bytes);
    });

    let bytes = receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("stdout reaches EOF once massorca exits; the service still holds the pipe");
    let _ = child.wait();
    assert!(String::from_utf8_lossy(&bytes).contains("\"pid\""));
}

#[test]
fn restart_five_times_in_a_row_starts_a_new_service_each_time() {
    let sandbox = Sandbox::new();
    let pids: Vec<Value> = (0..5)
        .map(|_| sandbox.json_ok("default", &["service", "restart"])["pid"].clone())
        .collect();

    let distinct: BTreeSet<String> = pids.iter().map(Value::to_string).collect();
    assert_eq!(
        distinct.len(),
        5,
        "every restart replaced the service: {pids:?}"
    );
    let status = sandbox.wait_for_running("default", true);
    assert_eq!(status["info"]["pid"], pids[4]);
}
