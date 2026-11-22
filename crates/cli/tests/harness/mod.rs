//! Shared harness for the CLI tests.
//!
//! Every test gets a temporary home. It reaches the `massorca` child through that child's
//! environment only, and a `Sandbox` stops the services of the profiles it used when dropped,
//! including one autostarted so late that it comes up after its first stop.
//!
//! Child output goes to temporary files, not pipes: an autostarted service inherits the caller's
//! handles on Windows, and a pipe would then stay open until the service stops. The regression
//! test is `autostart_does_not_keep_the_callers_stdout_pipe_open` in `service_lifecycle.rs`.
#![allow(dead_code)] // each test binary uses a different subset of these helpers

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::assert::Assert;
use massorca_cli::service::parent_watch::PARENT_PID_ENV;
use serde_json::Value;
use tempfile::TempDir;

/// Generous bounds: CI machines can be slow to start a service process.
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const STOP_TIMEOUT: Duration = Duration::from_secs(60);
/// Covers a late service's own ten-second wait for a lock that nothing answers for (ADR 0003).
const RELEASE_TIMEOUT: Duration = Duration::from_secs(30);
pub const WAIT: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(100);

/// Variables a test must not inherit from the developer's shell.
const CLEARED_ENV: [&str; 5] = [
    "MASSORCA_PROFILE",
    "MASSORCA_NO_AUTOSTART",
    "MASSORCA_LOG",
    "MASSORCA_HOST",
    "MASSORCA_LISTEN",
];

/// How many lines of a service log a failed run prints.
const SERVICE_LOG_TAIL_LINES: usize = 200;

/// The `massorca` binary with `home` on its environment and no profile flag. Its services keep vault keys in a file
/// inside the temporary profile, never in the developer's keyring.
pub fn massorca_in(home: &Path) -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("massorca"));
    command.env("MASSORCA_HOME", home);
    command.env("MASSORCA_VAULT_PROTECTION", "file");
    for name in CLEARED_ENV {
        command.env_remove(name);
    }
    // Services this command starts stop once this test process is gone, even when its sandbox never drops.
    command.env(PARENT_PID_ENV, std::process::id().to_string());
    command
}

/// Runs `command` to completion with its stdout and stderr in temporary files, and returns its output.
/// Returns `None` after `timeout`, once the command has been killed.
pub fn capture(command: &mut Command, timeout: Duration) -> Option<Output> {
    capture_with(command, Stdio::null(), timeout)
}

/// `capture` with `stdin` as the command's standard input.
pub fn capture_with(command: &mut Command, stdin: Stdio, timeout: Duration) -> Option<Output> {
    let mut stdout = tempfile::tempfile().expect("a temporary file for stdout");
    let mut stderr = tempfile::tempfile().expect("a temporary file for stderr");
    let mut child = command
        .stdin(stdin)
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

/// Runs `command` with `input` on its standard input, to completion within `COMMAND_TIMEOUT`.
pub fn execute_with_stdin(command: &mut Command, input: &str) -> Assert {
    let mut stdin = tempfile::tempfile().expect("a temporary file for stdin");
    stdin
        .write_all(input.as_bytes())
        .expect("the input can be written");
    stdin
        .seek(SeekFrom::Start(0))
        .expect("the input can be rewound");
    let output = capture_with(command, Stdio::from(stdin), COMMAND_TIMEOUT)
        .unwrap_or_else(|| panic!("the command did not finish in time: {command:?}"));
    Assert::new(output)
}

/// A temporary home, and the profiles whose services it stops when dropped.
#[derive(Debug)]
pub struct Sandbox {
    _temp: TempDir,
    /// A private home below the temporary folder, which may be shared with other users (ADR 0073).
    home: PathBuf,
    profiles: RefCell<BTreeSet<String>>,
    /// Variables every command of this sandbox sets, besides the harness's own.
    env: RefCell<Vec<(String, String)>>,
}

impl Sandbox {
    pub fn new() -> Self {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let home = temp.path().join("home");
        massorca_profile::create_home(&home).expect("a private home");
        Self {
            _temp: temp,
            home,
            profiles: RefCell::new(BTreeSet::new()),
            env: RefCell::new(Vec::new()),
        }
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn profile_dir(&self, profile: &str) -> PathBuf {
        self.home().join("profiles").join(profile)
    }

    /// Registers `profile` so its service is stopped when the sandbox drops.
    pub fn track(&self, profile: &str) {
        self.profiles.borrow_mut().insert(profile.to_owned());
    }

    /// Sets `name` for every command this sandbox runs from now on.
    pub fn set_env(&self, name: &str, value: &str) {
        self.env
            .borrow_mut()
            .push((name.to_owned(), value.to_owned()));
    }

    /// `massorca --profile <profile>` on this home.
    pub fn command(&self, profile: &str) -> Command {
        self.track(profile);
        let mut command = massorca_in(self.home());
        command.arg("--profile").arg(profile);
        for (name, value) in self.env.borrow().iter() {
            command.env(name, value);
        }
        command
    }

    pub fn run(&self, profile: &str, args: &[&str]) -> Assert {
        let mut command = self.command(profile);
        command.args(args);
        let output = self.output_of(profile, &mut command, Stdio::null());
        self.settle(profile, args, output)
    }

    /// Runs `args` with `input` as standard input, read from a file so no pipe outlives the command.
    pub fn run_with_input(&self, profile: &str, args: &[&str], input: &[u8]) -> Assert {
        let mut file = tempfile::tempfile().expect("a temporary file for stdin");
        std::io::Write::write_all(&mut file, input).expect("stdin is written");
        file.seek(SeekFrom::Start(0)).expect("stdin can be rewound");
        let mut command = self.command(profile);
        command.args(args);
        let output = self.output_of(profile, &mut command, Stdio::from(file));
        self.settle(profile, args, output)
    }

    /// Runs `--json <args>`, which must succeed, and returns the document's `result`. A failure also names the tail of
    /// the profile's service log, so a start that was refused, failed or timed out explains itself.
    pub fn json_ok(&self, profile: &str, args: &[&str]) -> Value {
        let mut command = self.command(profile);
        command.args(with_json(args));
        let output = self.output_of(profile, &mut command, Stdio::null());
        let outcome = self
            .settle(profile, args, output)
            .try_success()
            .unwrap_or_else(|error| panic!("{error}"));
        single_document(&outcome)["result"].clone()
    }

    /// Runs `command` to completion. A timeout panics with the tail of the profile's service log.
    fn output_of(&self, profile: &str, command: &mut Command, stdin: Stdio) -> Output {
        capture_with(command, stdin, COMMAND_TIMEOUT).unwrap_or_else(|| {
            panic!(
                "the command did not finish in time: {command:?}\n{}",
                self.service_log_tail(profile)
            )
        })
    }

    /// The outcome of a run. A failed run that may have started the profile's service names the tail of that service's
    /// log in its failure message, so a start that was refused, failed or timed out explains itself. A run with
    /// `--no-autostart` starts nothing.
    fn settle(&self, profile: &str, args: &[&str], output: Output) -> Assert {
        let assert = Assert::new(output);
        if !assert.get_output().status.success() && !args.contains(&"--no-autostart") {
            return assert.append_context("service log", self.service_log_tail(profile));
        }
        assert
    }

    /// The last `SERVICE_LOG_TAIL_LINES` lines of the newest service log of `profile`, for a failure message.
    pub fn service_log_tail(&self, profile: &str) -> String {
        let logs = self.profile_dir(profile).join("logs");
        let newest = fs::read_dir(&logs)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("service.") && name.ends_with(".log"))
            })
            .max();
        let Some(path) = newest else {
            return format!("no service log in {}", logs.display());
        };
        let text = fs::read(&path)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let tail = &lines[lines.len().saturating_sub(SERVICE_LOG_TAIL_LINES)..];
        format!(
            "service log {} (last {} of {} lines):\n{}",
            path.display(),
            tail.len(),
            lines.len(),
            tail.join("\n")
        )
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

impl Sandbox {
    /// Runs `service stop` for `profile`, ignoring its outcome.
    fn stop_service(&self, profile: &str) {
        let mut command = massorca_in(self.home());
        command
            .arg("--profile")
            .arg(profile)
            .args(["service", "stop"]);
        let _ = capture(&mut command, STOP_TIMEOUT);
    }

    /// Stops a service of `profile` that came up after the first stop, killing it when it does not stop.
    #[cfg(not(target_os = "linux"))]
    fn stop_late_service(&self, profile: &str) {
        let dir = self.profile_dir(profile);
        if lock_is_held(&dir) {
            self.stop_service(profile);
        }
        if lock_is_held(&dir) {
            kill_recorded_service(&dir);
        }
    }

    /// Waits until no process of this sandbox serves or still starts a service of `profiles`. A process holds its
    /// profile directory as working directory and keeps files open in it, and Windows refuses to rename a directory
    /// in that state, so a successful rename proves the profile free.
    #[cfg(windows)]
    fn release(&self, profiles: &BTreeSet<String>, deadline: Instant) {
        for profile in profiles {
            let dir = self.profile_dir(profile);
            let released = dir.with_file_name(format!("{profile}.released"));
            loop {
                match std::fs::rename(&dir, &released) {
                    Ok(()) => break,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                    Err(_) if Instant::now() >= deadline => break,
                    Err(_) => self.stop_late_service(profile),
                }
                thread::sleep(POLL);
            }
        }
    }

    /// Kills every process whose environment names this sandbox's home, which only processes it started carry.
    #[cfg(target_os = "linux")]
    fn release(&self, _profiles: &BTreeSet<String>, deadline: Instant) {
        loop {
            let pids = processes_in_home(self.home());
            if pids.is_empty() || Instant::now() >= deadline {
                return;
            }
            for pid in pids {
                kill(pid);
            }
            thread::sleep(POLL);
        }
    }

    /// Without a way to see a service that is still starting, only one that came up late is stopped.
    #[cfg(not(any(windows, target_os = "linux")))]
    fn release(&self, profiles: &BTreeSet<String>, _deadline: Instant) {
        for profile in profiles {
            self.stop_late_service(profile);
        }
    }
}

impl Drop for Sandbox {
    /// Stops the services of the profiles this sandbox used, then waits until none of them runs or still starts.
    /// Best effort, and it never panics: a test may have stopped the services already, and a panic here would abort
    /// the test run.
    fn drop(&mut self) {
        let profiles = std::mem::take(self.profiles.get_mut());
        for profile in &profiles {
            self.stop_service(profile);
        }
        self.release(&profiles, Instant::now() + RELEASE_TIMEOUT);
    }
}

/// `true` while a process holds the service lock of the profile in `dir`.
fn lock_is_held(dir: &Path) -> bool {
    let Ok(file) = OpenOptions::new().read(true).open(dir.join("service.lock")) else {
        return false;
    };
    matches!(file.try_lock(), Err(TryLockError::WouldBlock))
}

/// Kills the process that the profile's `service.json` names. Only a pid this sandbox's own service recorded is trusted.
#[cfg(not(target_os = "linux"))]
fn kill_recorded_service(dir: &Path) {
    let pid = std::fs::read(dir.join("service.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|info| info["pid"].as_u64());
    if let Some(pid) = pid.and_then(|pid| u32::try_from(pid).ok()) {
        kill(pid);
    }
}

/// The processes, other than this one, whose environment sets `MASSORCA_HOME` to `home`.
#[cfg(target_os = "linux")]
fn processes_in_home(home: &Path) -> Vec<u32> {
    use std::os::unix::ffi::OsStrExt;

    let mut entry = b"MASSORCA_HOME=".to_vec();
    entry.extend_from_slice(home.as_os_str().as_bytes());
    let Ok(processes) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    processes
        .filter_map(Result::ok)
        .filter_map(|process| process.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| *pid != std::process::id())
        .filter(|pid| {
            std::fs::read(format!("/proc/{pid}/environ"))
                .is_ok_and(|environ| environ.split(|byte| *byte == 0).any(|var| var == entry))
        })
        .collect()
}

/// Kills the process `pid` by id, never by name.
#[cfg(unix)]
pub fn kill(pid: u32) {
    let _ = Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Kills the process `pid` by id, never by name.
#[cfg(windows)]
pub fn kill(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
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
    /// Guards a child the test spawned itself.
    pub fn new(child: Child) -> Self {
        Self(child)
    }

    /// Kills the child now, the way a killed test process dies: no destructor of its own runs.
    pub fn kill_now(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }

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

/// Runs git in `dir` with a fixed identity; panics on failure. Returns stdout as text.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Creates a git repository at `dir` with one commit that holds `README.md`.
pub fn git_repo(dir: &Path) {
    std::fs::create_dir_all(dir).expect("create repository dir");
    git(dir, &["init", "-q"]);
    std::fs::write(dir.join("README.md"), "hello\n").expect("write readme");
    git(dir, &["add", "README.md"]);
    git(dir, &["commit", "-q", "-m", "initial"]);
}
