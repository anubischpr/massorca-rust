//! `massorca open` through the CLI. The desktop app is `massorca-fake-desktop`, which records its arguments and exits once
//! the test releases it, so no test shows a window or starts the real app.

mod harness;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use harness::{Sandbox, eventually, single_document, stdout_text};
use serde_json::Value;

/// The stand-in for the desktop app, built next to this test.
const STAND_IN: &str = env!("CARGO_BIN_EXE_massorca-fake-desktop");
/// Points `massorca open` at the desktop app to start.
const DESKTOP_ENV: &str = "MASSORCA_DESKTOP_EXECUTABLE";
/// Where the stand-in records its arguments, and the file that releases it.
const RECORD_ENV: &str = "MASSORCA_FAKE_DESKTOP_RECORD";
const RELEASE_ENV: &str = "MASSORCA_FAKE_DESKTOP_RELEASE";
/// `open` returns at once, so its output closes long before this; an app holding the pipes would keep it open.
const OUTPUT_CLOSES_WITHIN: Duration = Duration::from_secs(20);
/// How long a finished test waits for its stand-in to leave.
const STAND_IN_LEAVES_WITHIN: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(20);

/// The stand-in's files, in a folder of their own outside the sandbox.
struct StandIn {
    _dir: tempfile::TempDir,
    record: PathBuf,
    release: PathBuf,
}

impl StandIn {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let record = dir.path().join("desktop-args.json");
        let release = dir.path().join("desktop-release");
        Self {
            _dir: dir,
            record,
            release,
        }
    }

    /// Makes every `massorca` command of `sandbox` start this stand-in as the desktop app.
    fn attach(&self, sandbox: &Sandbox) {
        sandbox.set_env(DESKTOP_ENV, STAND_IN);
        sandbox.set_env(RECORD_ENV, path_text(&self.record));
        sandbox.set_env(RELEASE_ENV, path_text(&self.release));
    }

    /// The arguments the stand-in recorded, once it has started.
    fn recorded_args(&self) -> Vec<String> {
        let text = eventually("the desktop app to record its arguments", || {
            fs::read_to_string(&self.record).ok()
        });
        serde_json::from_str(&text).expect("the recorded arguments are a JSON array of strings")
    }

    fn exit_marker(&self) -> PathBuf {
        self.release.with_extension("exited")
    }
}

impl Drop for StandIn {
    /// Releases the stand-in even when a test panics, and waits for it to leave before its folder is removed.
    fn drop(&mut self) {
        let _ = fs::write(&self.release, b"");
        if self.record.exists() {
            let deadline = Instant::now() + STAND_IN_LEAVES_WITHIN;
            while !self.exit_marker().exists() && Instant::now() < deadline {
                thread::sleep(POLL);
            }
        }
    }
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

/// Reads `pipe` to its end on a thread and sends the text once it closes.
fn read_in_background(mut pipe: impl Read + Send + 'static) -> Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        let _ = sender.send(String::from_utf8_lossy(&bytes).into_owned());
    });
    receiver
}

/// Runs `command` with its output piped and returns its status and stdout once the output closes. An app that inherited
/// the pipes would hold them open, so that leak fails here rather than waiting for the app to quit.
fn run_until_output_closes(command: &mut Command) -> (ExitStatus, String) {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the massorca command starts");
    let stdout = read_in_background(child.stdout.take().expect("piped stdout"));
    let stderr = read_in_background(child.stderr.take().expect("piped stderr"));
    let Ok(stdout) = stdout.recv_timeout(OUTPUT_CLOSES_WITHIN) else {
        let _ = child.kill();
        panic!(
            "massorca open kept stdout open for {OUTPUT_CLOSES_WITHIN:?}: it waited for the app, or the app inherited the pipe"
        );
    };
    let Ok(_) = stderr.recv_timeout(OUTPUT_CLOSES_WITHIN) else {
        let _ = child.kill();
        panic!("massorca open kept stderr open for {OUTPUT_CLOSES_WITHIN:?}");
    };
    let status = child.wait().expect("the command can be waited for");
    (status, stdout)
}

#[test]
fn open_starts_the_desktop_app_for_the_profile_and_returns_while_the_app_runs() {
    let sandbox = Sandbox::new();
    sandbox.prepare_profile("default");
    let stand_in = StandIn::new();
    stand_in.attach(&sandbox);
    let mut command = sandbox.command("default");
    command.args(["open", "--json"]);

    let (status, stdout) = run_until_output_closes(&mut command);

    assert!(status.success(), "open failed: {stdout}");
    let document: Value = serde_json::from_str(stdout.trim()).expect("one JSON document");
    assert_eq!(document["ok"], true, "{document}");
    assert_eq!(document["result"]["profile"], "default", "{document}");
    assert!(document["result"]["pid"].is_u64(), "{document}");
    assert_eq!(
        stand_in.recorded_args(),
        ["--profile", "default", "--home", path_text(sandbox.home())]
    );
}

#[test]
fn open_without_autostart_passes_it_on_and_prints_one_line() {
    let sandbox = Sandbox::new();
    sandbox.prepare_profile("default");
    let stand_in = StandIn::new();
    stand_in.attach(&sandbox);

    let outcome = sandbox
        .run("default", &["--no-autostart", "open"])
        .success();

    assert_eq!(
        stdout_text(&outcome),
        "Launched the MassOrca desktop app for profile default.\n"
    );
    assert_eq!(
        stand_in.recorded_args(),
        [
            "--profile",
            "default",
            "--home",
            path_text(sandbox.home()),
            "--no-autostart"
        ]
    );
}

#[test]
fn open_refuses_a_remote_host_before_it_starts_anything() {
    let sandbox = Sandbox::new();
    sandbox.prepare_profile("default");

    let outcome = sandbox
        .run("default", &["--json", "--host", "lab", "open"])
        .code(2);

    assert_eq!(
        single_document(&outcome)["error"]["code"],
        "cli.host_not_supported"
    );
}

#[test]
fn open_reports_a_missing_desktop_app_with_its_stable_code() {
    let sandbox = Sandbox::new();
    sandbox.prepare_profile("default");
    let scratch = tempfile::tempdir().expect("a temporary folder");
    let missing = scratch.path().join("massorca-desktop-missing");
    sandbox.set_env(DESKTOP_ENV, path_text(&missing));

    let outcome = sandbox.run("default", &["--json", "open"]).code(1);

    let document = single_document(&outcome);
    assert_eq!(document["ok"], false, "{document}");
    assert_eq!(document["error"]["code"], "desktop.not_found", "{document}");
}
