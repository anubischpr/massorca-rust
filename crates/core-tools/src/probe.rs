//! One tool's probe: find the program on the `PATH`, then ask it for its version.

use std::io::{self, Read};
use std::path::Path;
use std::process::{ChildStdout, Stdio};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant};

use massorca_core_terminal::shell::{EnvSnapshot, resolve_program};
use massorca_protocol::methods::service::{ToolInfo, ToolKind, ToolState};

/// Output past this many bytes is discarded: a version line is short.
const OUTPUT_BUDGET: u64 = 4096;
const POLL: Duration = Duration::from_millis(10);
/// How long the output may take to arrive after the tool has exited.
const OUTPUT_GRACE: Duration = Duration::from_secs(1);

/// A tool the service looks for. `version_args` of `None` means the tool is found but never started.
#[derive(Debug, Clone, Copy)]
pub struct ToolSpec {
    pub id: &'static str,
    pub kind: ToolKind,
    pub program: &'static str,
    pub version_args: Option<&'static [&'static str]>,
}

/// Probes one tool: missing when no file of that name is on the `PATH`; otherwise its version, or `unknown` when it
/// did not answer in time, failed, or printed no version. A shell is never started, because a shell may load the
/// user's profile.
pub fn probe_tool(spec: &ToolSpec, env: &EnvSnapshot, timeout: Duration) -> ToolInfo {
    let Some(path) = resolve_program(spec.program, env) else {
        return tool_info(spec, ToolState::Missing, None, None);
    };
    let path_text = Some(path.to_string_lossy().into_owned());
    let Some(args) = spec.version_args else {
        return tool_info(spec, ToolState::Present, None, path_text);
    };
    let version = run_version(&path, args, timeout)
        .ok()
        .and_then(|output| parse_version_output(&output));
    match version {
        Some(version) => tool_info(spec, ToolState::Present, Some(version), path_text),
        None => tool_info(spec, ToolState::Unknown, None, path_text),
    }
}

/// Probes every tool, answering in the order of `specs`. Each probe runs on its own thread, so the wait is the
/// slowest tool's, not the sum.
pub fn probe_tools(specs: &[ToolSpec], env: &EnvSnapshot, timeout: Duration) -> Vec<ToolInfo> {
    thread::scope(|scope| {
        let probes: Vec<_> = specs
            .iter()
            .map(|spec| scope.spawn(move || probe_tool(spec, env, timeout)))
            .collect();
        probes
            .into_iter()
            .zip(specs)
            .map(|(probe, spec)| {
                probe
                    .join()
                    .unwrap_or_else(|_| tool_info(spec, ToolState::Unknown, None, None))
            })
            .collect()
    })
}

/// The version in a tool's output: the first word that starts with a number and holds a dot.
pub fn parse_version_output(text: &str) -> Option<String> {
    text.split_whitespace().find_map(|word| {
        if !word.starts_with(|c: char| c.is_ascii_digit()) {
            return None;
        }
        let number: String = word
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        let number = number.trim_end_matches('.');
        number.contains('.').then(|| number.to_owned())
    })
}

fn tool_info(
    spec: &ToolSpec,
    state: ToolState,
    version: Option<String>,
    path: Option<String>,
) -> ToolInfo {
    ToolInfo {
        id: spec.id.to_owned(),
        kind: spec.kind,
        state,
        version,
        path,
    }
}

/// Runs `path` with `args` and returns its standard output once it exits. A process still running at the deadline is
/// killed, and every failure (spawn, timeout, non-zero exit) is an error: the caller reports it as unknown.
fn run_version(path: &Path, args: &[&str], timeout: Duration) -> io::Result<String> {
    let mut child = massorca_process::command(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let (sender, receiver) = mpsc::channel();
    let stdout = child.stdout.take();
    thread::spawn(move || read_output(stdout, sender));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait()? {
            Some(status) => break status,
            None if Instant::now() < deadline => thread::sleep(POLL),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "the probe timed out",
                ));
            }
        }
    };
    if !status.success() {
        return Err(io::Error::other("the tool exited with a failure"));
    }
    let output = receiver.recv_timeout(OUTPUT_GRACE).unwrap_or_default();
    Ok(String::from_utf8_lossy(&output).into_owned())
}

/// Sends the first bytes of a pipe as soon as they end, then drains the rest so the tool never blocks on a full pipe.
fn read_output(pipe: Option<ChildStdout>, sender: Sender<Vec<u8>>) {
    let Some(mut pipe) = pipe else {
        let _ = sender.send(Vec::new());
        return;
    };
    let mut buffer = Vec::new();
    let _ = (&mut pipe).take(OUTPUT_BUDGET).read_to_end(&mut buffer);
    let _ = sender.send(buffer);
    let _ = io::copy(&mut pipe, &mut io::sink());
}
