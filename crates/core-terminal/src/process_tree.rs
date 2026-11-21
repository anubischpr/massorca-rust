//! Ends everything a terminal started (decision 0031): a job object per terminal on
//! Windows, the session on Unix.

use std::io;
#[cfg(windows)]
use std::sync::{Mutex, MutexGuard, PoisonError};

#[cfg(target_os = "linux")]
use rustix::process::kill_process;
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

    /// Asks the terminal's processes to exit: SIGHUP, then SIGCONT so stopped jobs see it.
    /// Windows: no-op; the graceful step there is closing the pseudoconsole.
    pub fn hangup(&self) {
        #[cfg(target_os = "linux")]
        {
            signal_session(self.shell, Signal::HUP);
            signal_session(self.shell, Signal::CONT);
        }
        #[cfg(all(unix, not(target_os = "linux")))]
        {
            signal_group(self.shell, Signal::HUP);
            signal_group(self.shell, Signal::CONT);
        }
    }

    /// Kills every process of the terminal at once. Idempotent.
    /// Unix: SIGKILL to the session and the shell's group. Windows: drops the job handle.
    pub fn kill(&self) {
        #[cfg(target_os = "linux")]
        signal_session(self.shell, Signal::KILL);
        #[cfg(unix)]
        signal_group(self.shell, Signal::KILL);
        #[cfg(windows)]
        drop(lock(&self.job).take());
    }

    /// Live processes of the terminal, best effort; for tests and diagnostics.
    /// Empty on macOS, and on Windows once the job is gone.
    pub fn members(&self) -> Vec<u32> {
        #[cfg(target_os = "linux")]
        {
            session_members(self.shell)
                .into_iter()
                .filter_map(|pid| u32::try_from(pid).ok())
                .collect()
        }
        #[cfg(all(unix, not(target_os = "linux")))]
        {
            Vec::new()
        }
        #[cfg(windows)]
        {
            let job = lock(&self.job);
            let Some(job) = job.as_ref() else {
                return Vec::new();
            };
            job.query_process_id_list()
                .map(|pids| {
                    pids.into_iter()
                        .filter_map(|pid| u32::try_from(pid).ok())
                        .collect()
                })
                .unwrap_or_default()
        }
    }
}

#[cfg(windows)]
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Pid and state of one `/proc/<pid>/stat` line: the fields after the last `)`, so a
/// command name with spaces or parentheses cannot shift them.
#[cfg(target_os = "linux")]
fn parse_stat(stat: &str) -> Option<(char, i32)> {
    let after_comm = &stat[stat.rfind(')')? + 1..];
    let mut fields = after_comm.split_ascii_whitespace();
    let state = fields.next()?.chars().next()?;
    let _parent = fields.next()?;
    let _group = fields.next()?;
    let session = fields.next()?.parse().ok()?;
    Some((state, session))
}

/// Session id of a process, `None` once it is gone. Zombies report their state as `Z`.
#[cfg(target_os = "linux")]
fn session_of(pid: i32) -> Option<(char, i32)> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parse_stat(&stat)
}

#[cfg(target_os = "linux")]
fn session_members(session: i32) -> Vec<i32> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse::<i32>().ok())
        .filter(|&pid| {
            matches!(session_of(pid), Some((state, sid))
                if sid == session && state != 'Z' && state != 'X')
        })
        .collect()
}

/// Signals every live member of the session. Each pid is checked again just before its
/// signal, so a pid reused since the scan is left alone.
#[cfg(target_os = "linux")]
fn signal_session(session: i32, signal: Signal) {
    for pid in session_members(session) {
        if matches!(session_of(pid), Some((_, sid)) if sid == session) {
            signal_pid(pid, signal);
        }
    }
}

#[cfg(unix)]
fn signal_group(group: i32, signal: Signal) {
    if let Some(group) = Pid::from_raw(group)
        && let Err(error) = kill_process_group(group, signal)
    {
        report_unsignalled(error);
    }
}

#[cfg(target_os = "linux")]
fn signal_pid(pid: i32, signal: Signal) {
    if let Some(pid) = Pid::from_raw(pid)
        && let Err(error) = kill_process(pid, signal)
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

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::parse_stat;

    #[test]
    fn stat_session_is_read_after_the_command_name() {
        assert_eq!(
            parse_stat("42 (sh) S 7 42 42 0 -1 4194560"),
            Some(('S', 42))
        );
        assert_eq!(
            parse_stat("43 (my (odd) name) R 42 43 99 0 -1"),
            Some(('R', 99))
        );
        assert_eq!(parse_stat("44 (x) Z 1 44 44 0"), Some(('Z', 44)));
        assert_eq!(parse_stat("45 (truncated"), None);
    }
}
