//! Ends everything a terminal started (decision 0031): a job object per terminal on
//! Windows, the session on Unix.

use std::io;
use std::sync::{Mutex, MutexGuard, PoisonError};

#[cfg(target_os = "linux")]
use rustix::process::kill_process;
#[cfg(all(unix, not(target_os = "linux")))]
use rustix::process::kill_process_group;
#[cfg(unix)]
use rustix::process::{Pid, Signal, WaitId, WaitIdOptions, waitid};
#[cfg(windows)]
use std::os::windows::io::RawHandle;

/// The processes one terminal's shell started. Held by the service until the terminal closes.
pub struct ProcessTree {
    /// The shell's pid, which is also its session id: `portable-pty` makes it a session leader.
    #[cfg(unix)]
    shell: i32,
    /// What the session holds. One lock, so a scan never runs while the shell is reaped.
    #[cfg(unix)]
    session: Mutex<Session>,
    /// Dropping the only handle ends every process still in the job.
    #[cfg(windows)]
    job: Mutex<Option<win32job::Job>>,
}

/// The Unix side of the tree: what was seen while the shell's pid was still held.
#[cfg(unix)]
#[derive(Default)]
struct Session {
    /// Set by the reap. From then on the shell's pid may name a new process, so nothing scans.
    reaped: bool,
    /// Live session members at the last scan, identified by pid and start time (Linux).
    #[cfg(target_os = "linux")]
    members: Vec<Member>,
    /// Process groups to signal, the shell's and the foreground one (macOS, no `/proc`).
    #[cfg(not(target_os = "linux"))]
    groups: Vec<i32>,
}

/// A session member. A pid reused since the scan has another start time.
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Member {
    pid: i32,
    start: u64,
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
        Ok(Self {
            shell,
            session: Mutex::default(),
        })
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
    /// `foreground` is the pty's foreground group; see `observe`.
    /// Windows: no-op; the graceful step there is closing the pseudoconsole.
    pub fn hangup(&self, foreground: Option<i32>) {
        #[cfg(unix)]
        {
            self.signal(Signal::HUP, foreground);
            self.signal(Signal::CONT, foreground);
        }
        #[cfg(windows)]
        {
            let _ = foreground;
        }
    }

    /// Kills every process of the terminal at once. Idempotent, and harmless after the reap:
    /// then it signals only what was recorded before it.
    /// Unix: SIGKILL to the recorded session members. Windows: drops the job handle.
    pub fn kill(&self, foreground: Option<i32>) {
        #[cfg(unix)]
        self.signal(Signal::KILL, foreground);
        #[cfg(windows)]
        {
            let _ = foreground;
            drop(lock(&self.job).take());
        }
    }

    /// Live processes of the terminal, best effort; for tests and diagnostics.
    /// Empty on macOS, and on Windows once the job is gone.
    pub fn members(&self) -> Vec<u32> {
        #[cfg(target_os = "linux")]
        {
            let mut session = lock(&self.session);
            self.observe(&mut session, None);
            session
                .members
                .iter()
                .filter(|member| alive(member, self.shell))
                .filter_map(|member| u32::try_from(member.pid).ok())
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

    /// Reaps the shell with `reap` once it has exited. Unix: the exit is observed without
    /// reaping, so the shell's pid (its session and group id) stays held while the session
    /// is recorded; the reap then runs under the lock, and nothing scans after it (0031).
    #[cfg(unix)]
    pub(crate) fn reap_shell<T>(
        &self,
        foreground: impl FnOnce() -> Option<i32>,
        reap: impl FnOnce() -> io::Result<T>,
    ) -> io::Result<T> {
        await_exit_unreaped(self.shell)?;
        let foreground = foreground();
        let mut session = lock(&self.session);
        self.observe(&mut session, foreground);
        let status = reap()?;
        session.reaped = true;
        Ok(status)
    }

    /// Windows: the job outlives the shell; nothing to observe before the reap.
    #[cfg(windows)]
    pub(crate) fn reap_shell<T>(
        &self,
        _foreground: impl FnOnce() -> Option<i32>,
        reap: impl FnOnce() -> io::Result<T>,
    ) -> io::Result<T> {
        reap()
    }

    /// Records what the session holds now. Only before the reap: until then the shell's pid
    /// cannot be reused, so the session id still names the shell's own session.
    /// On macOS the foreground group is recorded too, since it need not be the shell's group.
    #[cfg(unix)]
    fn observe(&self, session: &mut Session, foreground: Option<i32>) {
        if session.reaped {
            return;
        }
        #[cfg(target_os = "linux")]
        {
            let _ = foreground;
            session.members = session_members(self.shell);
        }
        #[cfg(not(target_os = "linux"))]
        {
            for group in [Some(self.shell), foreground].into_iter().flatten() {
                if !session.groups.contains(&group) {
                    session.groups.push(group);
                }
            }
        }
    }

    /// Signals what the session holds: a scan first while the shell is unreaped, then only
    /// the recorded processes, each checked again just before its signal.
    #[cfg(unix)]
    fn signal(&self, signal: Signal, foreground: Option<i32>) {
        let mut session = lock(&self.session);
        self.observe(&mut session, foreground);
        #[cfg(target_os = "linux")]
        {
            for member in &session.members {
                if alive(member, self.shell) {
                    signal_pid(member.pid, signal);
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            for &group in &session.groups {
                signal_group(group, signal);
            }
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Blocks until the shell exits, leaving it unreaped: a zombie keeps its pid held.
#[cfg(unix)]
fn await_exit_unreaped(shell: i32) -> io::Result<()> {
    let pid = Pid::from_raw(shell)
        .ok_or_else(|| io::Error::other("shell pid is not a valid process id"))?;
    loop {
        match waitid(
            WaitId::Pid(pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOWAIT,
        ) {
            Ok(_) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

/// The Linux fields of one `/proc/<pid>/stat` line the tree needs.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProcStat {
    state: char,
    session: i32,
    start: u64,
}

/// Reads the fields after the last `)`, so a command name with spaces or parentheses cannot
/// shift them. Field numbers follow `proc(5)`.
#[cfg(target_os = "linux")]
fn parse_stat(stat: &str) -> Option<ProcStat> {
    let after_comm = &stat[stat.rfind(')')? + 1..];
    let mut fields = after_comm.split_ascii_whitespace();
    let state = fields.next()?.chars().next()?;
    // Field 4 (parent) and field 5 (group), then field 6: the session.
    let session = fields.nth(2)?.parse().ok()?;
    // Fields 7 to 21, then field 22: the start time.
    let start = fields.nth(15)?.parse().ok()?;
    Some(ProcStat {
        state,
        session,
        start,
    })
}

#[cfg(target_os = "linux")]
fn process_stat(pid: i32) -> Option<ProcStat> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parse_stat(&stat)
}

/// A member still in the session it was recorded in. Zombies have ended.
#[cfg(target_os = "linux")]
fn alive(member: &Member, session: i32) -> bool {
    matches!(process_stat(member.pid), Some(stat)
        if stat.start == member.start
            && stat.session == session
            && !matches!(stat.state, 'Z' | 'X'))
}

#[cfg(target_os = "linux")]
fn session_members(session: i32) -> Vec<Member> {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse::<i32>().ok())
        .filter_map(|pid| {
            let stat = process_stat(pid)?;
            (stat.session == session && !matches!(stat.state, 'Z' | 'X')).then_some(Member {
                pid,
                start: stat.start,
            })
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn signal_pid(pid: i32, signal: Signal) {
    if let Some(pid) = Pid::from_raw(pid)
        && let Err(error) = kill_process(pid, signal)
    {
        report_unsignalled(error);
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
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

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{ProcStat, parse_stat};

    #[test]
    fn stat_fields_are_read_after_the_command_name() {
        let plain = "42 (sh) S 7 42 42 0 -1 4194560 1 0 0 0 1 1 0 0 20 0 1 0 987654 0";
        assert_eq!(
            parse_stat(plain),
            Some(ProcStat {
                state: 'S',
                session: 42,
                start: 987_654,
            })
        );
        let odd = "43 (my (odd) name) R 42 43 99 0 -1 4194560 1 0 0 0 1 1 0 0 20 0 1 0 5";
        assert_eq!(
            parse_stat(odd),
            Some(ProcStat {
                state: 'R',
                session: 99,
                start: 5,
            })
        );
        assert_eq!(parse_stat("45 (truncated"), None);
    }
}
