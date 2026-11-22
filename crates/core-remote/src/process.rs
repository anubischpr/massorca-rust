//! Finds executables and runs child processes with captured output and a timeout.

use std::fs::Metadata;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

/// How often a running child is checked against its deadline.
const POLL_INTERVAL: Duration = Duration::from_millis(20);
/// The least time output pipes get to close once the child has exited.
const OUTPUT_GRACE: Duration = Duration::from_secs(1);
/// Most characters of command output that go into a message.
const EXCERPT_CHARS: usize = 300;

/// A child process that finished before its deadline.
#[derive(Debug)]
pub(crate) struct Captured {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

/// Why a child process did not finish with its output.
#[derive(Debug)]
pub(crate) enum CaptureError {
    Spawn(io::Error),
    Wait(io::Error),
    TimedOut,
}

/// Runs `command` with `stdin` (null when `None`), captures both outputs and kills the child
/// if it is still running at `timeout`. Opens no console window on Windows.
pub(crate) fn run_captured(
    mut command: Command,
    stdin: Option<&[u8]>,
    timeout: Duration,
) -> Result<Captured, CaptureError> {
    hide_window(&mut command);
    command
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(CaptureError::Spawn)?;
    if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
        let input = input.to_vec();
        // Written from its own thread so a large input cannot block the readers below.
        // A child that exits early breaks the pipe; its exit status reports that instead.
        thread::spawn(move || {
            let _ = pipe.write_all(&input);
        });
    }
    let stdout = child.stdout.take().map(read_in_background);
    let stderr = child.stderr.take().map(read_in_background);
    let deadline = Instant::now() + timeout;
    let status = wait_until(&mut child, deadline)?;
    Ok(Captured {
        status,
        stdout: collect(stdout, deadline)?,
        stderr: collect(stderr, deadline)?,
    })
}

/// The first executable called `name` on `PATH`.
pub(crate) fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    find_in_dirs(std::env::split_paths(&path), name)
}

/// The first executable called `name` among `dirs`, in order. Empty and relative entries are skipped: they would
/// resolve against the current directory, which may hold a planted `ssh` or `tailscale`.
pub(crate) fn find_in_dirs(dirs: impl IntoIterator<Item = PathBuf>, name: &str) -> Option<PathBuf> {
    dirs.into_iter()
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

/// The first of `candidates` that is an executable file.
pub(crate) fn first_executable(candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .copied()
        .map(PathBuf::from)
        .find(|candidate| is_executable(candidate))
}

/// The start of some command output, trimmed for a message; a placeholder when it is empty.
pub(crate) fn excerpt(output: &[u8]) -> String {
    let text = String::from_utf8_lossy(output);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "(no output)".to_owned();
    }
    let mut shown: String = trimmed.chars().take(EXCERPT_CHARS).collect();
    if trimmed.chars().count() > EXCERPT_CHARS {
        shown.push_str("...");
    }
    shown
}

fn wait_until(child: &mut Child, deadline: Instant) -> Result<ExitStatus, CaptureError> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
            Ok(None) => {
                stop(child);
                return Err(CaptureError::TimedOut);
            }
            Err(err) => {
                stop(child);
                return Err(CaptureError::Wait(err));
            }
        }
    }
}

/// Kills a child and reaps it.
fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn read_in_background(mut pipe: impl Read + Send + 'static) -> Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        let _ = sender.send(bytes);
    });
    receiver
}

/// Waits for a background read; a pipe still open past the deadline counts as a timeout.
fn collect(output: Option<Receiver<Vec<u8>>>, deadline: Instant) -> Result<Vec<u8>, CaptureError> {
    let Some(receiver) = output else {
        return Ok(Vec::new());
    };
    let wait = deadline
        .saturating_duration_since(Instant::now())
        .max(OUTPUT_GRACE);
    receiver
        .recv_timeout(wait)
        .map_err(|_| CaptureError::TimedOut)
}

fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && has_execute_bit(&metadata))
}

#[cfg(unix)]
fn has_execute_bit(metadata: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn has_execute_bit(_metadata: &Metadata) -> bool {
    true
}

/// Stops a console program from opening a window on Windows. Does nothing elsewhere.
#[cfg(windows)]
fn hide_window(command: &mut Command) {
    massorca_process::creation_flags(command, 0);
}

/// Stops a console program from opening a window on Windows. Does nothing elsewhere.
#[cfg(not(windows))]
pub fn hide_window(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[cfg(unix)]
    fn shell(script: &str) -> Command {
        let mut command = massorca_process::command("/bin/sh");
        command.args(["-c", script]);
        command
    }

    #[cfg(unix)]
    #[test]
    fn stdin_is_delivered_and_a_large_stdout_is_read_completely() {
        let input: Vec<u8> = (0..1_500_000_u32).map(|i| (i % 251) as u8).collect();
        let captured = run_captured(shell("cat"), Some(&input), Duration::from_secs(60))
            .expect("cat finishes");
        assert!(captured.status.success());
        assert!(captured.stdout == input, "stdout differs from stdin");
    }

    #[cfg(unix)]
    #[test]
    fn stderr_and_the_exit_code_come_back_separately() {
        let captured = run_captured(
            shell("echo oops >&2; exit 4"),
            None,
            Duration::from_secs(30),
        )
        .expect("sh finishes");
        assert_eq!(captured.status.code(), Some(4));
        assert_eq!(captured.stderr, b"oops\n");
        assert!(captured.stdout.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_child_still_running_at_the_deadline_is_stopped() {
        let started = Instant::now();
        let result = run_captured(shell("sleep 30"), None, Duration::from_millis(200));
        assert!(matches!(result, Err(CaptureError::TimedOut)), "{result:?}");
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[cfg(windows)]
    #[test]
    fn a_child_still_running_at_the_deadline_is_stopped() {
        let started = Instant::now();
        let mut command = massorca_process::command("ping");
        command.args(["-n", "30", "127.0.0.1"]);
        let result = run_captured(command, None, Duration::from_millis(200));
        assert!(matches!(result, Err(CaptureError::TimedOut)), "{result:?}");
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn a_missing_program_is_a_spawn_error() {
        let result = run_captured(
            massorca_process::command("massorca-no-such-program"),
            None,
            Duration::from_secs(5),
        );
        assert!(matches!(result, Err(CaptureError::Spawn(_))), "{result:?}");
    }

    #[test]
    fn executables_are_found_among_the_given_directories() {
        let dir = tempfile::tempdir().expect("temp dir");
        let tool = dir.path().join(exe_name("fake-tool"));
        fs::write(&tool, b"not a real program").expect("write the fake tool");
        make_executable(&tool);
        assert_eq!(
            find_in_dirs([dir.path().to_owned()], &exe_name("fake-tool")),
            Some(tool)
        );
        assert_eq!(
            find_in_dirs([dir.path().to_owned()], &exe_name("absent-tool")),
            None
        );
    }

    #[test]
    fn empty_and_relative_path_entries_are_skipped() {
        // A directory under the current one, named relatively, like a `.` or empty PATH entry would be.
        let dir = tempfile::tempdir_in(".").expect("temp dir in the current directory");
        let tool = dir.path().join(exe_name("planted-tool"));
        fs::write(&tool, b"not a real program").expect("write the planted tool");
        make_executable(&tool);
        let relative = PathBuf::from(dir.path().file_name().expect("a name"));
        assert!(
            relative.join(exe_name("planted-tool")).is_file(),
            "reachable relatively"
        );
        assert_eq!(
            find_in_dirs(
                [PathBuf::new(), PathBuf::from("."), relative],
                &exe_name("planted-tool")
            ),
            None
        );
        let absolute = std::path::absolute(dir.path()).expect("absolute");
        assert_eq!(
            find_in_dirs(
                [PathBuf::from("."), absolute.clone()],
                &exe_name("planted-tool")
            ),
            Some(absolute.join(exe_name("planted-tool")))
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_file_without_the_execute_bit_is_not_an_executable() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join("plain-file"), b"data").expect("write the file");
        assert_eq!(find_in_dirs([dir.path().to_owned()], "plain-file"), None);
    }

    #[test]
    fn excerpt_trims_and_shortens_output() {
        assert_eq!(excerpt(b"  boom \n"), "boom");
        assert_eq!(excerpt(b" \n"), "(no output)");
        let long = "x".repeat(400);
        let shown = excerpt(long.as_bytes());
        assert_eq!(shown.len(), EXCERPT_CHARS + 3);
        assert!(shown.ends_with("..."));
    }

    fn exe_name(base: &str) -> String {
        if cfg!(windows) {
            format!("{base}.exe")
        } else {
            base.to_owned()
        }
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod the tool");
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &Path) {}
}
