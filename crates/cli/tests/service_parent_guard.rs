//! A service a test process started stops once that process is killed, although the killed process never runs the
//! `Sandbox` drop that would stop it.

mod harness;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

use harness::{ChildGuard, Sandbox, eventually, execute, kill, massorca_in, single_document};

/// Where the helper process writes its sandbox home and its service's pid.
const HELPER_DIR_ENV: &str = "MASSORCA_PARENT_GUARD_DIR";
/// Covers the service's poll of its parent and its shutdown.
const EXIT_BOUND: Duration = Duration::from_secs(30);

#[test]
fn a_service_started_by_a_killed_test_process_stops_without_its_sandbox_drop() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let mut helper = ChildGuard::new(
        massorca_process::command(std::env::current_exe().expect("the test binary"))
            .args(["--exact", "parent_guard_helper", "--ignored", "--nocapture"])
            .env(HELPER_DIR_ENV, dir.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the helper process starts"),
    );
    let home = PathBuf::from(eventually("the helper's sandbox home", || {
        fs::read_to_string(dir.path().join("home")).ok()
    }));
    let service_pid: u32 = eventually("the helper's service pid", || {
        fs::read_to_string(dir.path().join("service.pid"))
            .ok()?
            .trim()
            .parse()
            .ok()
    });

    helper.kill_now();

    let deadline = Instant::now() + EXIT_BOUND;
    while service_runs(&home) {
        if Instant::now() >= deadline {
            // Leaves no service behind even when the guard failed.
            kill(service_pid);
            panic!("the service outlived its killed test process for {EXIT_BOUND:?}");
        }
        thread::sleep(Duration::from_millis(200));
    }
    let _ = fs::remove_dir_all(home.parent().unwrap_or(&home));
}

/// `true` while the default profile's service in `home` answers `service status` as running.
fn service_runs(home: &Path) -> bool {
    let outcome =
        execute(massorca_in(home).args(["--json", "--profile", "default", "service", "status"]))
            .success();
    single_document(&outcome)["result"]["running"] == true
}

/// Not a test of its own: the test process that the test above starts and kills. It autostarts a service, reports the
/// service and its sandbox home, then waits. Its sandbox never drops.
#[test]
#[ignore = "started and killed by a_service_started_by_a_killed_test_process_stops_without_its_sandbox_drop"]
fn parent_guard_helper() {
    let Some(dir) = std::env::var_os(HELPER_DIR_ENV) else {
        return;
    };
    let dir = PathBuf::from(dir);
    let sandbox = Sandbox::new();
    let info = sandbox.json_ok("default", &["service", "info"]);
    fs::write(dir.join("home"), sandbox.home().display().to_string()).expect("the home is written");
    fs::write(dir.join("service.pid"), info["pid"].to_string()).expect("the pid is written");
    // The killing test ends this sleep long before its bound, which only stops a helper nobody kills.
    thread::sleep(Duration::from_secs(600));
}
