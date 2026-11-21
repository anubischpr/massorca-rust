//! Shared harness for the CLI tests.
//!
//! Every test gets a temporary home. It reaches the `massorca` child through that child's
//! environment only, and a `Sandbox` stops the services of the profiles it used when dropped.
//!
//! Child output goes to temporary files, not pipes: an autostarted service inherits the caller's
//! handles on Windows, and a pipe would then stay open until the service stops. The regression
//! test is `autostart_does_not_keep_the_callers_stdout_pipe_open` in `service_lifecycle.rs`.
#![allow(dead_code)] // each test binary uses a different subset of these helpers

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::assert::Assert;
use serde_json::Value;
use tempfile::TempDir;

/// Generous bounds: CI machines can be slow to start a service process.
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const STOP_TIMEOUT: Duration = Duration::from_secs(60);
pub const WAIT: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(100);

/// Variables a test must not inherit from the developer's shell.
const CLEARED_ENV: [&str; 3] = ["MASSORCA_PROFILE", "MASSORCA_NO_AUTOSTART", "MASSORCA_LOG"];

/// The `massorca` binary with `home` on its environment and no profile flag.
pub fn massorca_in(home: &Path) -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("massorca"));
    command.env("MASSORCA_HOME", home);
    for name in CLEARED_ENV {
        command.env_remove(name);
    }
    command
}

/// Runs `command` to completion with its stdout and stderr in temporary files, and returns its output.
/// Returns `None` after `timeout`, once the command has been killed.
pub fn capture(command: &mut Command, timeout: Duration) -> Option<Output> {
    let mut stdout = tempfile::tempfile().expect("a temporary file for stdout");
    let mut stderr = tempfile::tempfile().expect("a temporary file for stderr");
    let mut child = command
        .stdin(Stdio::null())
        .stdout(stdout.try_clone().expect("stdout can be duplicated"))
        .stderr(stderr.try_clone().expect("stderr can be duplicated"))
        .spawn()
        .expect("the massorca process starts");

    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait().expect("the process can be polled") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        thread::sleep(POLL);
    };
    Some(Output {
        status,
        stdout: read_back(&mut stdout),
        stderr: read_back(&mut stderr),
    })
}

fn read_back(file: &mut File) -> Vec<u8> {
    file.seek(SeekFrom::Start(0))
        .expect("the output file can be rewound");
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .expect("the output file can be read");
    bytes
}

/// Runs `command` to completion within `COMMAND_TIMEOUT` and returns its outcome for assertions.
pub fn execute(command: &mut Command) -> Assert {
    let output = capture(command, COMMAND_TIMEOUT)
        .unwrap_or_else(|| panic!("the command did not finish in time: {command:?}"));
    Assert::new(output)
}

/// A temporary home, and the profiles whose services it stops when dropped.
#[derive(Debug)]
pub struct Sandbox {
    home: TempDir,
    profiles: RefCell<BTreeSet<String>>,
}

impl Sandbox {
    pub fn new() -> Self {
        Self {
            home: tempfile::tempdir().expect("a temporary home"),
            profiles: RefCell::new(BTreeSet::new()),
        }
    }

    pub fn home(&self) -> &Path {
        self.home.path()
    }

    pub fn profile_dir(&self, profile: &str) -> PathBuf {
        self.home().join("profiles").join(profile)
    }

    /// Registers `profile` so its service is stopped when the sandbox drops.
    pub fn track(&self, profile: &str) {
        self.profiles.borrow_mut().insert(profile.to_owned());
    }

    /// `massorca --profile <profile>` on this home.
    pub fn command(&self, profile: &str) -> Command {
        self.track(profile);
        let mut command = massorca_in(self.home());
        command.arg("--profile").arg(profile);
        command
    }

    pub fn run(&self, profile: &str, args: &[&str]) -> Assert {
        execute(self.command(profile).args(args))
    }

    /// Runs `--json <args>`, which must succeed, and returns the document's `result`.
    pub fn json_ok(&self, profile: &str, args: &[&str]) -> Value {
        let outcome = self.run(profile, &with_json(args)).success();
        single_document(&outcome)["result"].clone()
    }

    /// The result of `service status`.
    pub fn status(&self, profile: &str) -> Value {
        self.json_ok(profile, &["service", "status"])
    }

    /// Polls `service status` until `running` matches, and returns that status.
    pub fn wait_for_running(&self, profile: &str, running: bool) -> Value {
        let deadline = Instant::now() + WAIT;
        loop {
            let status = self.status(profile);
            if status["running"] == running {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "profile {profile}: running={running} was not reached; last status {status}"
            );
            thread::sleep(POLL);
        }
    }

    /// Starts `massorca service run` as a child process in the foreground, with output discarded.
    pub fn spawn_service(&self, profile: &str) -> ChildGuard {
        self.spawn_service_with(profile, &[], Stdio::null(), Stdio::null())
    }

    /// Starts `massorca service run <extra_args>` as a child process with the given stdout and stderr.
    pub fn spawn_service_with(
        &self,
        profile: &str,
        extra_args: &[&str],
        stdout: Stdio,
        stderr: Stdio,
    ) -> ChildGuard {
        self.track(profile);
        let mut command = massorca_in(self.home());
        let child = command
            .args(["--profile", profile, "service", "run"])
            .args(extra_args)
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .spawn()
            .expect("the service process starts");
        ChildGuard(child)
    }
}

impl Drop for Sandbox {
    /// Stops the services of the profiles this sandbox used. Best effort, and it never panics:
    /// a test may have stopped the services already, and a panic here would abort the test run.
    fn drop(&mut self) {
        let profiles = std::mem::take(self.profiles.get_mut());
        for profile in profiles {
            let mut command = massorca_in(self.home());
            command
                .arg("--profile")
                .arg(&profile)
                .args(["service", "stop"]);
            let _ = capture(&mut command, STOP_TIMEOUT);
        }
    }
}

/// A spawned child, killed when the guard drops unless it already exited.
#[derive(Debug)]
pub struct ChildGuard(Child);

impl ChildGuard {
    /// Waits for the child to exit on its own; fails after `WAIT`.
    pub fn wait_for_exit(&mut self) -> ExitStatus {
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(status) = self.0.try_wait().expect("the child can be polled") {
                return status;
            }
            assert!(Instant::now() < deadline, "the child did not exit in time");
            thread::sleep(POLL);
        }
    }
}

impl ChildGuard {
    /// `true` while the child has not exited.
    pub fn is_running(&mut self) -> bool {
        matches!(self.0.try_wait(), Ok(None))
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

/// `args` with `--json` in front.
pub fn with_json<'a>(args: &[&'a str]) -> Vec<&'a str> {
    std::iter::once("--json")
        .chain(args.iter().copied())
        .collect()
}

/// The stdout of a command, as text.
pub fn stdout_text(outcome: &Assert) -> String {
    String::from_utf8_lossy(&outcome.get_output().stdout).into_owned()
}

/// The stdout of a command parsed as its one JSON document; fails unless stdout is exactly one line of JSON.
pub fn single_document(outcome: &Assert) -> Value {
    let stdout = stdout_text(outcome);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "stdout must hold exactly one JSON document: {stdout:?}"
    );
    serde_json::from_str(lines[0])
        .unwrap_or_else(|error| panic!("stdout is not JSON ({error}): {stdout:?}"))
}

/// Calls `attempt` until it yields a value; fails with `what` after `WAIT`.
pub fn eventually<T>(what: &str, mut attempt: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(value) = attempt() {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(POLL);
    }
}
