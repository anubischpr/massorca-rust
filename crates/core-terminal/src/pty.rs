//! Spawns a program in a pseudo-terminal (decision 0030): ConPTY on Windows, a Unix pty elsewhere.
//!
//! Blocking IO only: the caller reads on one thread and waits on another. Windows keeps
//! the output pipe open after the child exits, so the exit comes from `PtyChild::wait`.

use std::io::{self, Read, Write};
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError, TryLockError};

use portable_pty::{CommandBuilder, native_pty_system};

use crate::process_tree::ProcessTree;

/// Terminal size in character cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtySize {
    pub cols: u16,
    pub rows: u16,
}

/// What to run. `env` is the whole environment: the child starts with it alone.
#[derive(Clone)]
pub struct SpawnSpec {
    /// Absolute; resolved by the caller (`shell::resolve_program`), never searched here.
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub size: PtySize,
}

// Values can hold secrets, so `Debug` shows the variable names only.
impl std::fmt::Debug for SpawnSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.env.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("SpawnSpec")
            .field("program", &self.program)
            .field("args", &self.args)
            .field("cwd", &self.cwd)
            .field("env_names", &names)
            .field("size", &self.size)
            .finish()
    }
}

/// A started program: its terminal, its output, its exit and its process tree.
#[derive(Debug)]
pub struct Spawned {
    pub master: PtyMaster,
    pub reader: PtyReader,
    pub child: PtyChild,
    pub tree: ProcessTree,
}

#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("the program path is not absolute: {0}")]
    ProgramNotAbsolute(PathBuf),
    #[error("the working directory does not exist: {0}")]
    CwdNotFound(PathBuf),
    #[error("cannot open a pseudo-terminal: {0}")]
    Open(String),
    #[error("cannot start the program: {0}")]
    Spawn(String),
    #[error("cannot resize the terminal: {0}")]
    Resize(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Starts `spec` in a new pseudo-terminal and puts it in a new process tree. When an
/// error is returned, the program has been killed.
pub fn spawn(spec: &SpawnSpec) -> Result<Spawned, PtyError> {
    if !spec.program.is_absolute() {
        return Err(PtyError::ProgramNotAbsolute(spec.program.clone()));
    }
    if !spec.cwd.is_dir() {
        return Err(PtyError::CwdNotFound(spec.cwd.clone()));
    }
    #[cfg(unix)]
    check_executable(&spec.program, &spec.cwd)?;
    let pair = native_pty_system()
        .openpty(to_pty_size(spec.size))
        .map_err(|error| PtyError::Open(error.to_string()))?;

    let mut command = CommandBuilder::new(&spec.program);
    command.args(&spec.args);
    command.cwd(&spec.cwd);
    command.env_clear();
    for (name, value) in &spec.env {
        command.env(name, value);
    }
    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|error| PtyError::Spawn(error.to_string()))?;
    // Our copy of the slave must close, or the output never ends once the child is gone.
    drop(pair.slave);

    let tree = match attach_tree(child.as_mut()) {
        Ok(tree) => tree,
        Err(error) => {
            let _ = child.kill();
            return Err(PtyError::Spawn(error.to_string()));
        }
    };
    let streams = pair
        .master
        .try_clone_reader()
        .and_then(|reader| pair.master.take_writer().map(|writer| (reader, writer)));
    let (reader, writer) = match streams {
        Ok(streams) => streams,
        Err(error) => {
            tree.kill(None);
            return Err(PtyError::Open(error.to_string()));
        }
    };

    Ok(Spawned {
        master: PtyMaster {
            pid: child.process_id(),
            master: Mutex::new(Some(pair.master)),
            writer: Mutex::new(Some(writer)),
            closed: AtomicBool::new(false),
        },
        reader: PtyReader { inner: reader },
        child: PtyChild { inner: child },
        tree,
    })
}

fn to_pty_size(size: PtySize) -> portable_pty::PtySize {
    portable_pty::PtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// portable-pty closes every descriptor above 2 before `execve`, std's error pipe
/// included, so a failed exec shows up as a SIGABRT death instead of an error. Check
/// what `execve` needs first: the program, and the interpreter a `#!` line names.
#[cfg(unix)]
fn check_executable(program: &Path, cwd: &Path) -> Result<(), PtyError> {
    check_executable_file(program)?;
    let Some(interpreter) = shebang_interpreter(program) else {
        return Ok(());
    };
    // The kernel resolves a relative interpreter against the child's working directory.
    check_executable_file(&cwd.join(interpreter))
}

#[cfg(unix)]
fn check_executable_file(path: &Path) -> Result<(), PtyError> {
    let refused = |reason: String| PtyError::Spawn(format!("{}: {reason}", path.display()));
    let metadata = std::fs::metadata(path).map_err(|error| refused(error.to_string()))?;
    if !metadata.is_file() {
        return Err(refused("not a regular file".to_owned()));
    }
    rustix::fs::access(path, rustix::fs::Access::EXEC_OK)
        .map_err(|error| refused(format!("not executable ({error})")))
}

/// The interpreter of a `#!` line, read the way Linux does (its first 256 bytes).
/// An unreadable file is left to `execve`: a binary needs only the execute bit.
#[cfg(unix)]
fn shebang_interpreter(program: &Path) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let mut head = Vec::with_capacity(256);
    std::fs::File::open(program)
        .ok()?
        .take(256)
        .read_to_end(&mut head)
        .ok()?;
    let line = head.strip_prefix(b"#!")?;
    let line = line.split(|&byte| byte == b'\n').next()?;
    let interpreter = line
        .split(|byte| byte.is_ascii_whitespace())
        .find(|token| !token.is_empty())?;
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(interpreter)))
}

/// Puts the shell in its tree before it can start children of its own.
#[cfg(unix)]
fn attach_tree(child: &mut dyn portable_pty::Child) -> io::Result<ProcessTree> {
    let pid = child
        .process_id()
        .ok_or_else(|| io::Error::other("the shell has no process id"))?;
    ProcessTree::for_session(pid)
}

#[cfg(windows)]
fn attach_tree(child: &mut dyn portable_pty::Child) -> io::Result<ProcessTree> {
    let handle = child
        .as_raw_handle()
        .ok_or_else(|| io::Error::other("the shell has no process handle"))?;
    ProcessTree::for_job(handle)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn closed_error() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "the terminal is closed")
}

/// The terminal's control end: input, size, and closing it.
pub struct PtyMaster {
    /// The program's pid, for display and diagnostics.
    pid: Option<u32>,
    master: Mutex<Option<Box<dyn portable_pty::MasterPty + Send>>>,
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    closed: AtomicBool,
}

impl std::fmt::Debug for PtyMaster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtyMaster")
            .field("pid", &self.pid)
            .field("closed", &self.closed.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}

impl PtyMaster {
    /// Writes `bytes` to the program's input and flushes them.
    pub fn write_all(&self, bytes: &[u8]) -> io::Result<()> {
        if self.closed.load(Ordering::Acquire) {
            return Err(closed_error());
        }
        let mut writer = lock(&self.writer);
        let writer = writer.as_mut().ok_or_else(closed_error)?;
        writer.write_all(bytes)?;
        writer.flush()
    }

    pub fn resize(&self, size: PtySize) -> Result<(), PtyError> {
        let master = lock(&self.master);
        let master = master
            .as_ref()
            .ok_or_else(|| PtyError::Resize("the terminal is closed".to_owned()))?;
        master
            .resize(to_pty_size(size))
            .map_err(|error| PtyError::Resize(error.to_string()))
    }

    /// The program's pid, when the platform reports one.
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Drops the pseudoconsole or pty master. Idempotent; writes fail afterwards.
    /// Windows: `ClosePseudoConsole` sends `CTRL_CLOSE_EVENT` to the attached processes,
    /// and before Windows 11 24H2 it can block until the output is drained, so callers
    /// run this off the reader thread. Unix: the reader keeps its own copy of the
    /// master, so closing does not hang the program up; end it with `ProcessTree::hangup`.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        drop(lock(&self.master).take());
        // A write in progress holds the writer lock and can block on a full pty; the
        // closed flag refuses the next write, and the writer goes when `self` drops.
        match self.writer.try_lock() {
            Ok(mut writer) => drop(writer.take()),
            Err(TryLockError::Poisoned(poisoned)) => drop(poisoned.into_inner().take()),
            Err(TryLockError::WouldBlock) => {}
        }
    }

    /// The process group that holds the terminal's foreground (Unix; `None` on Windows).
    pub fn foreground_process_group(&self) -> Option<i32> {
        #[cfg(unix)]
        {
            lock(&self.master).as_ref()?.process_group_leader()
        }
        #[cfg(windows)]
        {
            None
        }
    }
}

/// The program's output. End of output reads as `Ok(0)`.
pub struct PtyReader {
    inner: Box<dyn Read + Send>,
}

impl std::fmt::Debug for PtyReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtyReader").finish_non_exhaustive()
    }
}

impl Read for PtyReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.inner.read(buf) {
            // Linux reports EIO once the slave is gone and its output is drained (0030).
            #[cfg(unix)]
            Err(error) if error.raw_os_error() == Some(rustix::io::Errno::IO.raw_os_error()) => {
                Ok(0)
            }
            other => other,
        }
    }
}

/// How the program ended. A signal death has no exit code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitInfo {
    pub code: Option<i64>,
    pub signal: Option<String>,
}

/// The program's process handle; `wait` is how it is reaped.
#[derive(Debug)]
pub struct PtyChild {
    inner: Box<dyn portable_pty::Child + Send + Sync>,
}

impl PtyChild {
    /// Blocks until the program exits.
    pub fn wait(&mut self) -> io::Result<ExitInfo> {
        let status = self.inner.wait()?;
        Ok(match status.signal() {
            Some(name) => ExitInfo {
                code: None,
                signal: Some(name.to_owned()),
            },
            None => ExitInfo {
                code: Some(i64::from(status.exit_code())),
                signal: None,
            },
        })
    }
}
