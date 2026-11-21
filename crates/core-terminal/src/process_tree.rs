//! Ends everything a terminal started (decision 0031): a job object per terminal on
//! Windows, the session on Unix.

use std::io;
#[cfg(windows)]
use std::sync::{Mutex, MutexGuard, PoisonError};

#[cfg(unix)]
use rustix::process::{Pid, Signal, kill_process_group};
#[cfg(windows)]
use std::os::windows::io::RawHandle;

/// The processes one terminal's shell started. Held by the service until the terminal closes.
pub struct ProcessTree {
    /// The shell's pid, which is also its session id: `portable-pty` makes it a session leader.
    #[cfg(unix)]
    shell: i32,
    /// Dropping the only handle ends every process still in the job.
    #[cfg(windows)]
    job: Mutex<Option<win32job::Job>>,
}

impl std::fmt::Debug for ProcessTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessTree").finish_non_exhaustive()
    }
}

impl ProcessTree {
    /// Takes the shell's session. Valid while the shell's pid is not reaped; `pty::spawn` calls it first.
    #[cfg(unix)]
    pub(crate) fn for_session(shell: u32) -> io::Result<Self> {
        let shell = i32::try_from(shell).map_err(|_| io::Error::other("shell pid out of range"))?;
        if Pid::from_raw(shell).is_none() {
            return Err(io::Error::other("shell pid is not a valid process id"));
        }
        Ok(Self { shell })
    }

    /// Creates the terminal's job and puts the shell in it. The job never allows breakaway (0031).
    #[cfg(windows)]
    pub(crate) fn for_job(shell: RawHandle) -> io::Result<Self> {
        let mut limits = win32job::ExtendedLimitInfo::new();
        limits.limit_kill_on_job_close();
        let job = win32job::Job::create_with_limit_info(&limits)?;
        job.assign_process(shell as isize)?;
        Ok(Self {
            job: Mutex::new(Some(job)),
        })
    }

    /// Kills the terminal's processes at once. Idempotent.
    /// Unix: SIGKILL to the shell's process group. Windows: drops the job handle.
    pub fn kill(&self) {
        #[cfg(unix)]
        signal_group(self.shell, Signal::KILL);
        #[cfg(windows)]
        drop(lock(&self.job).take());
    }
}

#[cfg(windows)]
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(unix)]
fn signal_group(group: i32, signal: Signal) {
    if let Some(group) = Pid::from_raw(group)
        && let Err(error) = kill_process_group(group, signal)
    {
        report_unsignalled(error);
    }
}

/// A process that already exited is the expected race; anything else is worth a trace.
#[cfg(unix)]
fn report_unsignalled(error: rustix::io::Errno) {
    if error != rustix::io::Errno::SRCH {
        tracing::warn!(error = ?error, "a terminal process could not be signalled");
    }
}
