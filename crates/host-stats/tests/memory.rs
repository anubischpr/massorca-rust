//! The reader against real processes: this test process and a child it starts (decision 0081).

use std::process::Child;
use std::time::{Duration, Instant};

use massorca_host_stats::{METRIC, Metric, descendants, memory_of, processes, sample_memory};

/// Set in the child's environment: the `sleeper` test then sleeps instead of returning.
const SLEEPER_ENV: &str = "MASSORCA_HOST_STATS_SLEEPER";
const TIMEOUT: Duration = Duration::from_secs(30);

/// Not a test of its own: the child process the other tests read. Returns at once unless started as the child.
#[test]
fn sleeper() {
    if std::env::var_os(SLEEPER_ENV).is_some() {
        std::thread::sleep(Duration::from_secs(120));
    }
}

/// The test binary itself, running only `sleeper`, killed (by its pid) when dropped.
struct Sleeper(Child);

impl Sleeper {
    fn start() -> Self {
        let exe = std::env::current_exe().expect("the test binary");
        let child = massorca_process::command(exe)
            .args(["--exact", "sleeper", "--test-threads", "1"])
            .env(SLEEPER_ENV, "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("the sleeper starts");
        Self(child)
    }

    fn pid(&self) -> u32 {
        self.0.id()
    }
}

impl Drop for Sleeper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn eventually<T>(what: &str, mut attempt: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(value) = attempt() {
            return value;
        }
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A pid no process holds: the largest a pid can be on Linux and macOS, and far above any Windows pid in use.
const NO_SUCH_PID: u32 = 0x3FFF_FFF0;

#[test]
fn the_metric_is_the_working_set_on_windows_and_the_resident_set_elsewhere() {
    let expected = if cfg!(windows) {
        Metric::WorkingSet
    } else {
        Metric::Resident
    };
    assert_eq!(METRIC, expected);
}

#[test]
fn this_process_uses_memory() {
    let bytes = memory_of(std::process::id()).expect("this process can be read");
    assert!(bytes > 0, "{bytes}");
}

#[test]
fn a_started_child_uses_memory_and_is_listed_under_this_process() {
    let child = Sleeper::start();
    let bytes = eventually("the child's memory is read", || {
        memory_of(child.pid()).filter(|bytes| *bytes > 0)
    });
    assert!(bytes > 0);

    let table = processes();
    let entry = table
        .iter()
        .find(|entry| entry.pid == child.pid())
        .expect("the child is in the process table");
    assert_eq!(entry.parent, Some(std::process::id()));
    assert!(!entry.name.is_empty(), "{entry:?}");
    let tree = descendants(std::process::id(), &table);
    assert!(tree.contains(&std::process::id()), "{tree:?}");
    assert!(tree.contains(&child.pid()), "{tree:?}");
}

#[test]
fn a_pid_that_does_not_exist_is_unreadable_and_does_not_fail_the_sample() {
    assert_eq!(memory_of(NO_SUCH_PID), None);

    let own = std::process::id();
    let sample = sample_memory(&[own, NO_SUCH_PID, own]);

    assert_eq!(sample.metric, METRIC);
    assert_eq!(sample.unreadable, 1);
    assert_eq!(sample.processes.len(), 1, "{sample:?}");
    assert_eq!(sample.processes[0].pid, own);
    assert!(sample.processes[0].bytes > 0);
    assert_eq!(sample.total_bytes(), sample.processes[0].bytes);
}

#[test]
fn an_exited_child_is_unreadable() {
    let mut child = Sleeper::start();
    let pid = child.pid();
    eventually("the child is read before it ends", || memory_of(pid));
    // Killed but not reaped: the child's handle (Windows) or its zombie (Unix) keeps the pid from being reused.
    child.0.kill().expect("the child is killed");
    eventually("the ended child is no longer read", || {
        memory_of(pid).is_none().then_some(())
    });
    let sample = sample_memory(&[pid]);
    assert!(sample.processes.is_empty(), "{sample:?}");
    assert_eq!(sample.unreadable, 1);
}
