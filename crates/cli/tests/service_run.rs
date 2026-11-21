//! `massorca service run`: the service in this process, its start document and its log files.

mod harness;

use std::fs;
use std::process::Stdio;

use harness::{Sandbox, eventually, single_document};
use predicates::str::contains;

/// Waits until the service of the default profile logs its start, so the test knows it holds the profile.
fn wait_for_start_log(sandbox: &Sandbox) {
    let logs = sandbox.profile_dir("default").join("logs");
    eventually("the start line in a service log file", || {
        fs::read_dir(&logs)
            .ok()?
            .filter_map(Result::ok)
            .find(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.starts_with("service.")
                    && name.ends_with(".log")
                    && fs::read_to_string(entry.path())
                        .is_ok_and(|text| text.contains("service started"))
            })
    });
}

#[test]
fn service_run_logs_its_start_to_daily_files_in_the_profile_logs_directory() {
    let sandbox = Sandbox::new();
    let _service = sandbox.spawn_service("default");

    wait_for_start_log(&sandbox);
}

#[test]
fn foreground_service_prints_one_start_document_and_logs_to_stderr_on_request() {
    let sandbox = Sandbox::new();
    let stdout_path = sandbox.home().join("service-run.stdout");
    let stderr_path = sandbox.home().join("service-run.stderr");
    let stdout = fs::File::create(&stdout_path).expect("a file for stdout");
    let stderr = fs::File::create(&stderr_path).expect("a file for stderr");
    let _service = sandbox.spawn_service_with(
        "default",
        &["--json", "--log-stderr"],
        Stdio::from(stdout),
        Stdio::from(stderr),
    );

    eventually("the start document on stdout", || {
        fs::read_to_string(&stdout_path)
            .ok()
            .filter(|text| text.ends_with('\n'))
    });
    let printed = fs::read_to_string(&stdout_path).expect("the service stdout");
    let lines: Vec<&str> = printed.lines().collect();
    assert_eq!(lines.len(), 1, "one start document, got: {printed:?}");
    let doc: serde_json::Value = serde_json::from_str(lines[0]).expect("the start document");
    assert_eq!(doc["ok"], true);
    assert_eq!(doc["result"]["profile"], "default");
    let logged = fs::read_to_string(&stderr_path).expect("the service stderr");
    assert!(logged.contains("service started"), "stderr log: {logged:?}");
}

#[test]
fn second_service_run_on_a_served_profile_exits_3_and_leaves_the_first_running() {
    let sandbox = Sandbox::new();
    let mut service = sandbox.spawn_service("default");
    wait_for_start_log(&sandbox);

    sandbox
        .run("default", &["service", "run"])
        .code(3)
        .stderr(contains("service.already_running"));

    let outcome = sandbox
        .run("default", &["--json", "service", "run"])
        .code(3);
    assert_eq!(
        single_document(&outcome)["error"]["code"],
        "service.already_running"
    );
    assert!(service.is_running(), "the first service keeps running");
}
