//! `massorca service log path`: where the profile's local trace logs are, read on this computer without the service.

mod harness;

use std::fs;

use harness::{Sandbox, single_document};
use predicates::str::contains;

#[test]
fn log_path_names_the_logs_folder_and_the_newest_service_and_desktop_files() {
    let sandbox = Sandbox::new();
    sandbox.prepare_profile("default");
    let logs = sandbox.profile_dir("default").join("logs");
    fs::create_dir_all(&logs).expect("the logs folder");
    for name in [
        "service.2026-11-22.log",
        "service.2026-11-24.log",
        "desktop.2026-11-23.log",
    ] {
        fs::write(logs.join(name), "line\n").expect("a log file");
    }

    let outcome = sandbox
        .run("default", &["--json", "service", "log", "path"])
        .success();
    let result = &single_document(&outcome)["result"];
    let dir = result["dir"].as_str().expect("dir is a string");
    assert_eq!(
        fs::canonicalize(dir).expect("dir exists"),
        fs::canonicalize(&logs).expect("the logs folder exists")
    );
    assert_eq!(result["service"], "service.2026-11-24.log");
    assert_eq!(result["desktop"], "desktop.2026-11-23.log");

    sandbox
        .run("default", &["service", "log", "path"])
        .success()
        .stdout(contains("service  service.2026-11-24.log"))
        .stdout(contains("desktop  desktop.2026-11-23.log"));
}

#[test]
fn log_path_without_logs_reports_none_and_never_starts_the_service() {
    let sandbox = Sandbox::new();

    let outcome = sandbox
        .run("default", &["--json", "service", "log", "path"])
        .success();
    let result = &single_document(&outcome)["result"];
    assert!(
        result["dir"]
            .as_str()
            .is_some_and(|dir| dir.ends_with("logs"))
    );
    assert!(result["service"].is_null());
    assert!(result["desktop"].is_null());

    sandbox
        .run("default", &["service", "status"])
        .stdout(contains("not running"));
}

#[test]
fn log_path_refuses_a_remote_host() {
    let sandbox = Sandbox::new();
    sandbox
        .run(
            "default",
            &["--host", "remote-host", "service", "log", "path"],
        )
        .code(2)
        .stderr(contains("cli.host_not_supported"));
}
