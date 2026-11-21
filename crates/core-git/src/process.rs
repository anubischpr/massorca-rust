//! Runs one child process with piped output and an optional deadline. The child
//! is killed when it outlives the deadline; output is read on threads so a full pipe never
//! stalls it.

use std::io::{self, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// How long output may keep arriving after the child exits. A background process
/// that inherited the pipes must not hold the caller indefinitely.
const PIPE_GRACE: Duration = Duration::from_secs(2);

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug)]
pub(crate) struct Output {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

#[derive(Debug)]
pub(crate) enum Bounded {
    Finished(Output),
    /// Also returned when the child exited but its output never closed in time.
    TimedOut,
}

/// Runs `command` with stdin closed and both output streams captured. The stdio
/// and window settings are applied here, so no caller can run a child without them.
/// `None` waits for the child however long it runs.
pub(crate) fn run_bounded(command: &mut Command, timeout: Option<Duration>) -> io::Result<Bounded> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().map(read_on_thread);
    let stderr = child.stderr.take().map(read_on_thread);
    let deadline = timeout.and_then(|timeout| Instant::now().checked_add(timeout));
    let status = match wait_for_exit(&mut child, deadline) {
        Ok(Some(status)) => status,
        Ok(None) => {
            reap(&mut child);
            return Ok(Bounded::TimedOut);
        }
        Err(error) => {
            reap(&mut child);
            return Err(error);
        }
    };
    let pipes_deadline = Instant::now() + PIPE_GRACE;
    match (
        join_until(stdout, pipes_deadline)?,
        join_until(stderr, pipes_deadline)?,
    ) {
        (Some(stdout), Some(stderr)) => Ok(Bounded::Finished(Output {
            status,
            stdout,
            stderr,
        })),
        _ => Ok(Bounded::TimedOut),
    }
}

fn read_on_thread<R: Read + Send + 'static>(mut pipe: R) -> JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes).map(|_| bytes)
    })
}

/// Polls until the child exits. `None` means the deadline passed first; without a
/// deadline (no timeout, or one too large for an `Instant`) it waits for the exit.
fn wait_for_exit(child: &mut Child, deadline: Option<Instant>) -> io::Result<Option<ExitStatus>> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Ok(None);
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// Collects one output stream, or `None` when it is still open at `deadline`.
fn join_until(
    handle: Option<JoinHandle<io::Result<Vec<u8>>>>,
    deadline: Instant,
) -> io::Result<Option<Vec<u8>>> {
    let Some(handle) = handle else {
        return Ok(Some(Vec::new()));
    };
    while !handle.is_finished() {
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(POLL_INTERVAL);
    }
    match handle.join() {
        Ok(bytes) => bytes.map(Some),
        Err(_) => Err(io::Error::other("git output reader panicked")),
    }
}

/// Kills and collects a child; both calls fail harmlessly when it has already exited.
fn reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::process::Command;
    use std::time::{Duration, Instant};

    use super::{Bounded, run_bounded};

    /// Set only on the child process, never on the test process.
    const CHILD_SLEEPS: &str = "MASSORCA_CORE_GIT_TEST_CHILD_SLEEPS";
    const CHILD_TEST: &str = "process::tests::child_sleeps_when_asked";

    /// This test binary limited to `CHILD_TEST`: a real child process on every OS.
    fn test_binary() -> Command {
        let mut command = Command::new(std::env::current_exe().expect("test binary path"));
        command.args(["--exact", CHILD_TEST, "--nocapture"]);
        command
    }

    /// Does nothing unless a test below re-runs this binary with `CHILD_SLEEPS` set.
    #[test]
    fn child_sleeps_when_asked() {
        if std::env::var_os(CHILD_SLEEPS).is_some() {
            std::thread::sleep(Duration::from_secs(120));
        }
    }

    #[test]
    fn a_finished_child_returns_its_status_and_output() {
        let outcome =
            run_bounded(&mut test_binary(), Some(Duration::from_secs(120))).expect("spawn");
        match outcome {
            Bounded::Finished(output) => {
                assert!(output.status.success());
                assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
            }
            Bounded::TimedOut => panic!("a finished child was reported as timed out"),
        }
    }

    #[test]
    fn a_child_that_outlives_the_timeout_is_killed_and_reported() {
        let mut command = test_binary();
        command.env(CHILD_SLEEPS, "1");
        let started = Instant::now();
        let outcome = run_bounded(&mut command, Some(Duration::from_millis(300))).expect("spawn");
        assert!(matches!(outcome, Bounded::TimedOut), "{outcome:?}");
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the child was not killed"
        );
    }

    #[test]
    fn a_missing_program_is_a_spawn_error() {
        let mut command = Command::new("massorca-no-such-program");
        let error =
            run_bounded(&mut command, Some(Duration::from_secs(5))).expect_err("no such program");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}
