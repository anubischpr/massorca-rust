//! Helpers shared by the terminal integration tests.
// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use massorca_core_terminal::pty::{
    ExitInfo, PtyChild, PtyMaster, PtyReader, PtySize, SpawnSpec, Spawned, spawn,
};
use massorca_core_terminal::shell::{EnvSnapshot, TerminalEnv, build_environment, resolve_program};

/// Generous so a loaded machine passes; waits end as soon as their condition holds.
pub const TIMEOUT: Duration = Duration::from_secs(30);
/// Wide enough that ConPTY does not wrap or redraw the lines the tests read.
pub const SIZE: PtySize = PtySize {
    cols: 200,
    rows: 50,
};

/// The platform's non-interactive shell: `sh` on Unix, `cmd.exe` on Windows.
pub fn shell_program() -> PathBuf {
    let name = if cfg!(windows) { "cmd.exe" } else { "sh" };
    resolve_program(name, &EnvSnapshot::current()).expect("the platform shell is on PATH")
}

/// The environment a terminal really gets, built from this process's environment.
pub fn terminal_env() -> Vec<(String, String)> {
    let extra = std::collections::BTreeMap::new();
    build_environment(
        &EnvSnapshot::current(),
        &TerminalEnv {
            terminal_id: "test-terminal",
            workspace_id: "test-workspace",
            service_version: "0.0.0-test",
            massorca_home: None,
            massorca_profile: None,
            extra: &extra,
        },
    )
}

pub fn spec(program: PathBuf, args: &[&str], cwd: &Path, env: Vec<(String, String)>) -> SpawnSpec {
    SpawnSpec {
        program,
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        cwd: cwd.to_path_buf(),
        env,
        size: SIZE,
    }
}

/// Removes escape sequences and carriage returns, leaving the text a reader would see.
pub fn visible_text(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            // CSI: parameters, then a final byte in '@'..='~'.
            Some('[') => {
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            // OSC: ends at BEL or ESC \.
            Some(']') => {
                let mut previous = ' ';
                for next in chars.by_ref() {
                    if next == '\u{7}' || (previous == '\u{1b}' && next == '\\') {
                        break;
                    }
                    previous = next;
                }
            }
            _ => {}
        }
    }
    out.replace('\r', "")
}

pub fn visible_lines(raw: &str) -> Vec<String> {
    visible_text(raw)
        .lines()
        .map(|line| line.trim().to_owned())
        .collect()
}

/// Output of one terminal, read on its own thread. Answers the cursor-position query
/// (`ESC [ 6 n`) that ConPTY sends before it renders anything.
pub struct Output {
    bytes: Arc<Mutex<Vec<u8>>>,
    finished: Receiver<()>,
}

pub fn capture(reader: PtyReader, master: Arc<PtyMaster>) -> Output {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let (finished_tx, finished) = mpsc::channel();
    let sink = Arc::clone(&bytes);
    thread::spawn(move || {
        let mut reader = reader;
        let mut chunk = [0_u8; 4096];
        let mut answered = 0;
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    let mut collected = sink.lock().expect("output lock");
                    collected.extend_from_slice(&chunk[..read]);
                    let queries = count(&collected, b"\x1b[6n");
                    while answered < queries {
                        let _ = master.write_all(b"\x1b[1;1R");
                        answered += 1;
                    }
                }
            }
        }
        let _ = finished_tx.send(());
    });
    Output { bytes, finished }
}

fn count(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

impl Output {
    pub fn raw(&self) -> String {
        String::from_utf8_lossy(&self.bytes.lock().expect("output lock")).into_owned()
    }

    /// Polls until `satisfied` holds for the visible text, or panics with what was seen.
    pub fn wait_until(&self, what: &str, satisfied: impl Fn(&str) -> bool) {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let text = visible_text(&self.raw());
            if satisfied(&text) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {what}; visible output: {text:?}"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    pub fn wait_for(&self, needle: &str) {
        self.wait_until(&format!("{needle:?}"), |text| text.contains(needle));
    }

    /// Polls until `value` finds the wanted text in the raw output, or panics with what was seen.
    pub fn wait_for_value(&self, what: &str, value: impl Fn(&str) -> Option<String>) -> String {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let text = self.raw();
            if let Some(found) = value(&text) {
                return found;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {what}: {text:?}"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// Waits until the reader ends: end of output on Unix, or after the pseudoconsole closes on Windows.
    pub fn finish(&self) {
        self.finished
            .recv_timeout(TIMEOUT)
            .expect("the output reader ends once the terminal closes");
    }
}

/// A running terminal: its control end, its captured output, and its exit.
pub struct Running {
    pub master: Arc<PtyMaster>,
    pub output: Output,
    pub exit: Receiver<ExitInfo>,
    /// Ends the terminal's processes on drop, so a failed assertion leaves none behind.
    end: Option<Box<dyn FnOnce() + Send>>,
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(end) = self.end.take() {
            end();
        }
    }
}

/// Starts `spec` with its output captured. The process tree lives as long as the `Running`.
pub fn start(spec: &SpawnSpec) -> Running {
    let Spawned {
        master,
        reader,
        child,
        tree,
    } = spawn(spec).expect("spawn the program in a pseudo-terminal");
    let master = Arc::new(master);
    let output = capture(reader, Arc::clone(&master));
    Running {
        exit: wait_for_exit(child),
        master,
        output,
        end: Some(Box::new(move || tree.kill(None))),
    }
}

pub fn wait_for_exit(mut child: PtyChild) -> Receiver<ExitInfo> {
    let (exit_tx, exit) = mpsc::channel();
    thread::spawn(move || {
        let status = child.wait().expect("wait for the program");
        let _ = exit_tx.send(status);
    });
    exit
}

/// Types `text` into the terminal, as a keyboard would.
pub fn type_text(master: &PtyMaster, text: &str) {
    master
        .write_all(text.as_bytes())
        .expect("write to the terminal");
}

pub fn exit_within(exit: &Receiver<ExitInfo>) -> ExitInfo {
    exit.recv_timeout(TIMEOUT)
        .expect("the program exits within the timeout")
}
