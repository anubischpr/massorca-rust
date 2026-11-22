//! Runs one child process with piped output and an optional deadline. On timeout
//! the child is killed with every process it started (hooks, ssh, credential
//! helpers); output is read on threads so a full pipe never stalls it.

use std::io::{self, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// How long output may keep arriving after the child exits, and how long the
/// readers get to finish once the process tree is killed. A background process
/// that inherited the pipes must not hold the caller indefinitely.
const PIPE_GRACE: Duration = Duration::from_secs(2);

/// How long a kill waits for the processes of a Windows job to exit.
#[cfg(windows)]
const KILL_WAIT: Duration = Duration::from_secs(10);

/// How long a finished command with no deadline waits for a pipe held outside its process tree.
const ESCAPED_GRACE: Duration = Duration::from_secs(30);

/// Git's messages are short; a child that floods stderr is cut here rather than filling memory.
const MAX_STDERR: usize = 1 << 20;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug)]
pub(crate) struct Output {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    /// Stdout reached the byte budget: it holds the first bytes only, and the process tree was killed.
    pub(crate) truncated: bool,
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
/// `None` waits for the child however long it runs. Past `max_stdout` bytes the
/// tree is killed and the output so far is returned as truncated.
pub(crate) fn run_bounded(
    command: &mut Command,
    timeout: Option<Duration>,
    max_stdout: Option<usize>,
) -> io::Result<Bounded> {
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
    let overflow = Arc::new(AtomicBool::new(false));
    let stdout = child
        .stdout
        .take()
        .map(|pipe| read_on_thread(pipe, max_stdout, Arc::clone(&overflow)));
    let stderr = child
        .stderr
        .take()
        .map(|pipe| read_on_thread(pipe, Some(MAX_STDERR), Arc::new(AtomicBool::new(false))));
    let readers = [&stdout, &stderr];
    let deadline = timeout.and_then(|timeout| Instant::now().checked_add(timeout));
    let status = match wait_for_exit(&mut child, deadline, &overflow) {
        Ok(Some(status)) => status,
        Ok(None) if overflow.load(Ordering::Acquire) => {
            tree.kill();
            reap(&mut child);
            wait_for_readers(&readers, Instant::now() + PIPE_GRACE);
            return Ok(Bounded::Finished(Output {
                status: child.wait()?,
                stdout: join(stdout)?,
                stderr: join(stderr)?,
                truncated: true,
            }));
        }
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
        // What still holds it now is outside the tree: on Unix, a process another thread forked and has not yet
        // exec'd keeps a copy of every pipe until it does, which on a loaded machine takes seconds. The command
        // finished, so wait up to its own deadline rather than calling it timed out.
        let until = deadline.map_or(Instant::now() + ESCAPED_GRACE, |deadline| {
            deadline.max(Instant::now() + PIPE_GRACE)
        });
        if !wait_for_readers(&readers, until) {
            return Ok(Bounded::TimedOut);
        }
    }
    Ok(Bounded::Finished(Output {
        status,
        stdout: join(stdout)?,
        stderr: join(stderr)?,
        truncated: overflow.load(Ordering::Acquire),
    }))
}

/// The child and every process it starts: a process group on Unix, a job object
/// on Windows. Processes the child detaches (a new session, a job breakaway) escape.
#[derive(Debug)]
struct ProcessTree {
    #[cfg(unix)]
    group: u32,
    /// `None` when the job could not be created; then only the child is killed.
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

    /// The child is created inside the job, so nothing it starts escapes the job.
    #[cfg(windows)]
    fn spawn(command: &mut Command) -> io::Result<(Child, Self)> {
        let (child, job) =
            massorca_core_remote::windows_job::spawn_in_job(command, CREATE_NO_WINDOW)?;
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
        // Termination is asynchronous: wait until the job is empty, so nothing git started (such as the `git branch`
        // a `worktree add -b` runs) can still write once the caller starts cleaning up after it.
        #[cfg(windows)]
        if let Some(job) = self.job {
            massorca_core_remote::windows_job::terminate_and_wait(&job, KILL_WAIT);
        }
    }

    /// Leaves processes the child left running alone, as a plain spawn would.
    fn release(self) {
        #[cfg(windows)]
        if let Some(job) = self.job {
            let _ = job.set_extended_limit_info(win32job::ExtendedLimitInfo::new().clear_limits());
        }
    }
}

/// Reads `pipe` to its end, or until `limit` bytes; then it stops reading and raises `overflow`.
fn read_on_thread<R: Read + Send + 'static>(
    pipe: R,
    limit: Option<usize>,
    overflow: Arc<AtomicBool>,
) -> Reader {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let Some(limit) = limit else {
            let mut pipe = pipe;
            return pipe.read_to_end(&mut bytes).map(|_| bytes);
        };
        // One byte past the limit tells a full budget from an overflow.
        let budget = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
        pipe.take(budget).read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            bytes.truncate(limit);
            overflow.store(true, Ordering::Release);
        }
        Ok(bytes)
    })
}

/// Polls until the child exits. `None` means the deadline passed first, or stdout
/// overflowed its budget; without either it waits for the exit.
fn wait_for_exit(
    child: &mut Child,
    deadline: Option<Instant>,
    overflow: &AtomicBool,
) -> io::Result<Option<ExitStatus>> {
    loop {
        if overflow.load(Ordering::Acquire) {
            return Ok(None);
        }
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

    use std::sync::atomic::AtomicBool;

    use super::{Bounded, run_bounded, wait_for_exit};

    /// Set only on child processes, never on the test process.
    const CHILD_SLEEPS: &str = "MASSORCA_CORE_GIT_TEST_CHILD_SLEEPS";
    /// Makes the child start a sleeping grandchild that shares its output, write the
    /// grandchild's pid to this file, then exit or sleep as `CHILD_THEN` says.
    const CHILD_SPAWNS: &str = "MASSORCA_CORE_GIT_TEST_CHILD_SPAWNS";
    const CHILD_THEN: &str = "MASSORCA_CORE_GIT_TEST_CHILD_THEN";
    const CHILD_TEST: &str = "process::tests::child_sleeps_when_asked";
    /// Makes the child write far more than any test budget to stdout, then sleep.
    const CHILD_FLOODS: &str = "MASSORCA_CORE_GIT_TEST_CHILD_FLOODS";

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
            let escape = std::env::var_os(CHILD_THEN).is_some_and(|then| then == "escape");
            let mut grandchild = test_binary();
            grandchild
                .env_remove(CHILD_SPAWNS)
                .env(CHILD_SLEEPS, if escape { "6" } else { "120" })
                .stdin(Stdio::null());
            // A grandchild in a process group of its own is outside the tree a kill reaches.
            #[cfg(unix)]
            if escape {
                use std::os::unix::process::CommandExt;
                grandchild.process_group(0);
            }
            let grandchild = grandchild.spawn().expect("spawn grandchild");
            std::fs::write(pid_file, grandchild.id().to_string()).expect("write pid");
            if std::env::var_os(CHILD_THEN).is_some_and(|then| then == "sleep") {
                std::thread::sleep(Duration::from_secs(120));
            }
            leave_running(grandchild);
        } else if let Some(seconds) = std::env::var_os(CHILD_SLEEPS) {
            let seconds = seconds
                .to_str()
                .and_then(|text| text.parse().ok())
                .unwrap_or(120);
            std::thread::sleep(Duration::from_secs(seconds));
        } else if std::env::var_os(CHILD_FLOODS).is_some() {
            use std::io::Write;
            let chunk = vec![b'x'; 64 * 1024];
            let mut stdout = std::io::stdout().lock();
            for _ in 0..1024 {
                if stdout.write_all(&chunk).is_err() {
                    return;
                }
            }
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
            run_bounded(&mut test_binary(), Some(Duration::from_secs(120)), None).expect("spawn");
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
        let outcome =
            run_bounded(&mut command, Some(Duration::from_millis(300)), None).expect("spawn");
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
        let outcome = run_bounded(&mut command, Some(Duration::from_secs(3)), None).expect("spawn");
        assert!(matches!(outcome, Bounded::TimedOut), "{outcome:?}");
        assert!(started.elapsed() < Duration::from_secs(30));
        assert_grandchild_dies(&pid_file);
    }

    #[test]
    fn a_background_child_holding_the_output_is_killed_and_the_run_still_succeeds() {
        let dir = tempfile::tempdir().expect("temp dir");
        let (mut command, pid_file) = child_with_grandchild("exit", dir.path());
        let started = Instant::now();
        let outcome =
            run_bounded(&mut command, Some(Duration::from_secs(120)), None).expect("spawn");
        match outcome {
            Bounded::Finished(output) => assert!(output.status.success()),
            Bounded::TimedOut => panic!("a successful child was reported as timed out"),
        }
        assert!(started.elapsed() < Duration::from_secs(30));
        assert_grandchild_dies(&pid_file);
    }

    /// The case a concurrent fork produces: the output stays held by a process the kill cannot reach, for a while.
    #[cfg(unix)]
    #[test]
    fn a_finished_child_whose_output_is_held_outside_its_tree_is_not_a_timeout() {
        let dir = tempfile::tempdir().expect("temp dir");
        let (mut command, _pid_file) = child_with_grandchild("escape", dir.path());
        let started = Instant::now();
        let outcome =
            run_bounded(&mut command, Some(Duration::from_secs(120)), None).expect("spawn");
        match outcome {
            Bounded::Finished(output) => assert!(output.status.success()),
            Bounded::TimedOut => panic!("a finished child was reported as timed out"),
        }
        assert!(started.elapsed() < Duration::from_secs(60));
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
        let outcome =
            wait_for_exit(&mut child, Some(Instant::now()), &AtomicBool::new(false)).expect("poll");
        assert!(outcome.is_none(), "{outcome:?}");
    }

    #[test]
    fn output_past_the_budget_is_cut_and_the_child_killed() {
        let mut command = test_binary();
        command.env(CHILD_FLOODS, "1");
        let started = Instant::now();
        let outcome =
            run_bounded(&mut command, Some(Duration::from_secs(120)), Some(4096)).expect("spawn");
        match outcome {
            Bounded::Finished(output) => {
                assert!(output.truncated);
                assert_eq!(output.stdout.len(), 4096);
            }
            Bounded::TimedOut => panic!("an overflow was reported as a timeout"),
        }
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the child was not killed"
        );
    }

    #[test]
    fn output_within_the_budget_is_not_truncated() {
        let outcome = run_bounded(
            &mut test_binary(),
            Some(Duration::from_secs(120)),
            Some(1 << 20),
        )
        .expect("spawn");
        match outcome {
            Bounded::Finished(output) => assert!(!output.truncated && output.status.success()),
            Bounded::TimedOut => panic!("timed out"),
        }
    }

    #[test]
    fn a_missing_program_is_a_spawn_error() {
        let mut command = Command::new("massorca-no-such-program");
        let error = run_bounded(&mut command, Some(Duration::from_secs(5)), None)
            .expect_err("no such program");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}
