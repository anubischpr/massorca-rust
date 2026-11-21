//! Runs one child process with piped output and an optional deadline. On timeout
//! the child is killed with every process it started (hooks, ssh, credential
//! helpers); output is read on threads so a full pipe never stalls it.

use std::io::{self, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// How long output may keep arriving after the child exits, and how long the
/// readers get to finish once the process tree is killed. A background process
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
    /// Also returned when the output stayed open even after the process tree was killed.
    TimedOut,
}

type Reader = JoinHandle<io::Result<Vec<u8>>>;

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
    let (mut child, tree) = ProcessTree::spawn(command)?;
    let stdout = child.stdout.take().map(read_on_thread);
    let stderr = child.stderr.take().map(read_on_thread);
    let readers = [&stdout, &stderr];
    let deadline = timeout.and_then(|timeout| Instant::now().checked_add(timeout));
    let status = match wait_for_exit(&mut child, deadline) {
        Ok(Some(status)) => status,
        outcome => {
            tree.kill();
            reap(&mut child);
            wait_for_readers(&readers, Instant::now() + PIPE_GRACE);
            return outcome.map(|_| Bounded::TimedOut);
        }
    };
    if wait_for_readers(&readers, Instant::now() + PIPE_GRACE) {
        tree.release();
    } else {
        // A process the child started still holds the output open.
        tree.kill();
        if !wait_for_readers(&readers, Instant::now() + PIPE_GRACE) {
            return Ok(Bounded::TimedOut);
        }
    }
    Ok(Bounded::Finished(Output {
        status,
        stdout: join(stdout)?,
        stderr: join(stderr)?,
    }))
}

/// The child and every process it starts: a process group on Unix, a job object
/// on Windows. Processes the child detaches (a new session, a job breakaway) escape.
#[derive(Debug)]
struct ProcessTree {
    #[cfg(unix)]
    group: u32,
    /// `None` when the job could not be created or assigned; then only the child is killed.
    #[cfg(windows)]
    job: Option<win32job::Job>,
}

impl ProcessTree {
    #[cfg(unix)]
    fn spawn(command: &mut Command) -> io::Result<(Child, Self)> {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
        let child = command.spawn()?;
        let group = child.id();
        Ok((child, Self { group }))
    }

    /// Processes the child starts before it joins the job escape it; assigning right
    /// after the spawn keeps that window to the child's own startup.
    #[cfg(windows)]
    fn spawn(command: &mut Command) -> io::Result<(Child, Self)> {
        use std::os::windows::io::AsRawHandle;
        let child = command.spawn()?;
        let job = win32job::Job::create_with_limit_info(
            win32job::ExtendedLimitInfo::new().limit_kill_on_job_close(),
        )
        .ok()
        .filter(|job| job.assign_process(child.as_raw_handle() as isize).is_ok());
        Ok((child, Self { job }))
    }

    /// Kills every process in the tree. Called before the child is reaped, so the
    /// group id cannot have been reused.
    fn kill(self) {
        #[cfg(unix)]
        {
            // A negative id names the process group. Without `--`, procps `kill`
            // reads it as an option and exits 0 having killed nothing.
            let _ = Command::new("kill")
                .args(["-s", "KILL", "--"])
                .arg(format!("-{}", self.group))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        // Closing the only handle to a kill-on-close job terminates its processes.
        #[cfg(windows)]
        drop(self.job);
    }

    /// Leaves processes the child left running alone, as a plain spawn would.
    fn release(self) {
        #[cfg(windows)]
        if let Some(job) = self.job {
            let _ = job.set_extended_limit_info(win32job::ExtendedLimitInfo::new().clear_limits());
        }
    }
}

fn read_on_thread<R: Read + Send + 'static>(mut pipe: R) -> Reader {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        pipe.read_to_end(&mut bytes).map(|_| bytes)
    })
}

/// Polls until the child exits. `None` means the deadline passed first; without a
/// deadline (no timeout, or one too large for an `Instant`) it waits for the exit.
fn wait_for_exit(child: &mut Child, deadline: Option<Instant>) -> io::Result<Option<ExitStatus>> {
    loop {
        // Checked first, so a passed deadline wins even over a child that already exited.
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Ok(None);
        }
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// Whether every reader finished before `deadline`. A reader still running then
/// ends on its own once its pipe closes.
fn wait_for_readers(readers: &[&Option<Reader>], deadline: Instant) -> bool {
    loop {
        if readers
            .iter()
            .all(|reader| reader.as_ref().is_none_or(JoinHandle::is_finished))
        {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// The bytes a finished reader collected; an absent stream is empty.
fn join(reader: Option<Reader>) -> io::Result<Vec<u8>> {
    match reader {
        None => Ok(Vec::new()),
        Some(reader) => reader
            .join()
            .map_err(|_| io::Error::other("git output reader panicked"))?,
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
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use super::{Bounded, run_bounded, wait_for_exit};

    /// Set only on child processes, never on the test process.
    const CHILD_SLEEPS: &str = "MASSORCA_CORE_GIT_TEST_CHILD_SLEEPS";
    /// Makes the child start a sleeping grandchild that shares its output, write the
    /// grandchild's pid to this file, then exit or sleep as `CHILD_THEN` says.
    const CHILD_SPAWNS: &str = "MASSORCA_CORE_GIT_TEST_CHILD_SPAWNS";
    const CHILD_THEN: &str = "MASSORCA_CORE_GIT_TEST_CHILD_THEN";
    const CHILD_TEST: &str = "process::tests::child_sleeps_when_asked";

    /// This test binary limited to `CHILD_TEST`: a real child process on every OS.
    fn test_binary() -> Command {
        let mut command = Command::new(std::env::current_exe().expect("test binary path"));
        command.args(["--exact", CHILD_TEST, "--nocapture"]);
        command
    }

    /// Does nothing unless a test below re-runs this binary with one of the variables set.
    #[test]
    fn child_sleeps_when_asked() {
        if let Some(pid_file) = std::env::var_os(CHILD_SPAWNS) {
            let mut grandchild = test_binary();
            grandchild
                .env_remove(CHILD_SPAWNS)
                .env(CHILD_SLEEPS, "1")
                .stdin(Stdio::null());
            let grandchild = grandchild.spawn().expect("spawn grandchild");
            std::fs::write(pid_file, grandchild.id().to_string()).expect("write pid");
            if std::env::var_os(CHILD_THEN).is_some_and(|then| then == "sleep") {
                std::thread::sleep(Duration::from_secs(120));
            }
            leave_running(grandchild);
        } else if std::env::var_os(CHILD_SLEEPS).is_some() {
            std::thread::sleep(Duration::from_secs(120));
        }
    }

    /// Exits without waiting for `child`: outliving its parent is the point of the test.
    fn leave_running(child: std::process::Child) {
        drop(child);
    }

    /// A child that starts a grandchild sharing its output; returns the command and pid file.
    fn child_with_grandchild(then: &str, dir: &Path) -> (Command, std::path::PathBuf) {
        let pid_file = dir.join("grandchild.pid");
        let mut command = test_binary();
        command.env(CHILD_SPAWNS, &pid_file).env(CHILD_THEN, then);
        (command, pid_file)
    }

    fn is_alive(pid: u32) -> bool {
        #[cfg(windows)]
        {
            let output = Command::new("tasklist")
                .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
                .output()
                .expect("run tasklist");
            String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\""))
        }
        #[cfg(unix)]
        {
            Command::new("kill")
                .args(["-0", &pid.to_string()])
                .stderr(Stdio::null())
                .status()
                .expect("run kill")
                .success()
        }
    }

    /// Waits up to ten seconds for the grandchild named in `pid_file` to be gone.
    fn assert_grandchild_dies(pid_file: &Path) {
        let pid: u32 = std::fs::read_to_string(pid_file)
            .expect("the child wrote the grandchild pid")
            .trim()
            .parse()
            .expect("pid");
        let deadline = Instant::now() + Duration::from_secs(10);
        while is_alive(pid) {
            assert!(
                Instant::now() < deadline,
                "grandchild {pid} outlived the run"
            );
            std::thread::sleep(Duration::from_millis(100));
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
    fn a_timeout_kills_the_children_of_the_child_too() {
        let dir = tempfile::tempdir().expect("temp dir");
        let (mut command, pid_file) = child_with_grandchild("sleep", dir.path());
        let started = Instant::now();
        let outcome = run_bounded(&mut command, Some(Duration::from_secs(3))).expect("spawn");
        assert!(matches!(outcome, Bounded::TimedOut), "{outcome:?}");
        assert!(started.elapsed() < Duration::from_secs(30));
        assert_grandchild_dies(&pid_file);
    }

    #[test]
    fn a_background_child_holding_the_output_is_killed_and_the_run_still_succeeds() {
        let dir = tempfile::tempdir().expect("temp dir");
        let (mut command, pid_file) = child_with_grandchild("exit", dir.path());
        let started = Instant::now();
        let outcome = run_bounded(&mut command, Some(Duration::from_secs(120))).expect("spawn");
        match outcome {
            Bounded::Finished(output) => assert!(output.status.success()),
            Bounded::TimedOut => panic!("a successful child was reported as timed out"),
        }
        assert!(started.elapsed() < Duration::from_secs(30));
        assert_grandchild_dies(&pid_file);
    }

    #[test]
    fn a_passed_deadline_wins_over_a_child_that_already_exited() {
        let mut child = test_binary()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn");
        child.wait().expect("the child exits");
        let outcome = wait_for_exit(&mut child, Some(Instant::now())).expect("poll");
        assert!(outcome.is_none(), "{outcome:?}");
    }

    #[test]
    fn a_missing_program_is_a_spawn_error() {
        let mut command = Command::new("massorca-no-such-program");
        let error =
            run_bounded(&mut command, Some(Duration::from_secs(5))).expect_err("no such program");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}
