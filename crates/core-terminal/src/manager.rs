//! `TerminalManager`: the service's terminals, their output and their attachments (decision 0033).
//!
//! Per terminal, a reader thread feeds the screen, the ring and the attachments under the
//! terminal's lock, and a waiter thread records the exit. Lock order: registry and terminal
//! locks are never held together; a terminal lock may be held while taking an attachment
//! queue lock, never the reverse. Nothing that can block (PTY writes, resize, close) runs
//! under a terminal lock.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::attach::{
    AttachItem, AttachMode, AttachShared, Attachment, MAX_ATTACHMENTS, QueryAnswers,
};
use crate::process_tree::ProcessTree;
use crate::pty::{self, ExitInfo, PtyChild, PtyError, PtyMaster, PtyReader, PtySize, SpawnSpec};
use crate::ring::{ByteRing, RING_CAPACITY};
use crate::screen::{FrozenScreen, Screen};
use crate::shell::{EnvSnapshot, TerminalEnv, build_environment, default_shell, resolve_program};
use massorca_host_stats::MemorySample;

/// Largest input accepted per call (0033).
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
const READ_CHUNK: usize = 64 * 1024;
/// How long the exit waits for the reader to reach end of output once the terminal is
/// closed (0030). Windows ends the output only when the pseudoconsole closes, so it
/// waits for the close to flush; on Unix end of output comes when the last holder of the
/// terminal goes, and a background process may hold it for good.
#[cfg(windows)]
const DRAIN_LIMIT: Duration = Duration::from_secs(5);
#[cfg(not(windows))]
const DRAIN_LIMIT: Duration = Duration::from_millis(500);
/// How long a kill waits for the exit after the tree is killed: the reap plus the drain,
/// so a kill returns the recorded exit.
const KILL_EXIT_MARGIN: Duration = Duration::from_secs(2).saturating_add(DRAIN_LIMIT);
/// The same wait during `shutdown`, short so it fits the service's shutdown budget; an exit
/// that takes longer is recorded after `shutdown` returns.
const SHUTDOWN_EXIT_MARGIN: Duration = Duration::from_secs(2);
/// After a failed wait the waiter tries again, first after this long, then doubling up to
/// `WAIT_RETRY_MAX`; a kill wakes it at once.
const WAIT_RETRY_FIRST: Duration = Duration::from_millis(250);
const WAIT_RETRY_MAX: Duration = Duration::from_secs(5);
/// The cursor position query (DSR 6). ConPTY renders nothing until it is answered.
const CURSOR_QUERY: &[u8] = b"\x1b[6n";

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

#[derive(Debug, Clone)]
pub struct ManagerConfig {
    /// The environment terminals inherit (0032).
    pub base_env: EnvSnapshot,
    pub service_version: String,
    pub massorca_home: Option<String>,
    pub massorca_profile: Option<String>,
    pub max_live: usize,
    pub max_exited: usize,
    /// How long a graceful kill waits before killing the tree (0031).
    pub kill_grace: Duration,
    /// The same wait during `shutdown`, shorter so it fits the service's 5 s shutdown budget (0033).
    pub shutdown_grace: Duration,
}

impl ManagerConfig {
    pub fn new(base_env: EnvSnapshot, service_version: impl Into<String>) -> Self {
        Self {
            base_env,
            service_version: service_version.into(),
            massorca_home: None,
            massorca_profile: None,
            max_live: 256,
            max_exited: 64,
            kill_grace: Duration::from_secs(5),
            shutdown_grace: Duration::from_secs(2),
        }
    }
}

/// The program a terminal is (0032). A bare name is searched in `PATH`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellChoice {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Clone)]
pub struct CreateSpec {
    pub workspace_id: String,
    pub cwd: PathBuf,
    /// `None` starts the default shell.
    pub shell: Option<ShellChoice>,
    /// Added to the environment last; may override anything.
    pub env: BTreeMap<String, String>,
    pub size: PtySize,
    pub title: Option<String>,
    /// A repeated id returns the first terminal instead of starting another.
    pub request_id: Option<String>,
    /// The program got vault secrets (decision 0071). Kept for the terminal's whole life in the registry, exited
    /// included: its scrollback may hold what was printed.
    pub holds_secrets: bool,
}

// Environment values can be secrets, so `Debug` shows the names only.
impl fmt::Debug for CreateSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CreateSpec")
            .field("workspace_id", &self.workspace_id)
            .field("cwd", &self.cwd)
            .field("shell", &self.shell)
            .field("env_names", &self.env.keys().collect::<Vec<_>>())
            .field("size", &self.size)
            .field("title", &self.title)
            .field("request_id", &self.request_id)
            .field("holds_secrets", &self.holds_secrets)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalState {
    Live,
    /// The wait for the process failed, so whether it runs is unknown. Never treated as
    /// death: it stays listed, counted as live, attachable, and accepts input, resize and
    /// kill. Only a later successful wait (a confirmed reap) makes it `Exited`; if the
    /// process can never be reaped it stays `Unverifiable` until closed.
    Unverifiable,
    Exited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExitReason {
    /// It ended on its own.
    Exited,
    Killed,
    ServiceStopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitRecord {
    pub code: Option<i64>,
    pub signal: Option<String>,
    pub reason: ExitReason,
    pub at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInfo {
    /// UUID v7.
    pub id: String,
    pub workspace_id: String,
    pub state: TerminalState,
    /// The explicit title, else the one the program set (OSC 0/2).
    pub title: Option<String>,
    pub cols: u16,
    pub rows: u16,
    pub pid: Option<u32>,
    /// The program's path.
    pub shell: String,
    pub cwd: String,
    pub created_at_ms: i64,
    pub exit: Option<ExitRecord>,
    /// Set at creation for a run that got vault secrets, and never cleared.
    pub holds_secrets: bool,
}

/// One terminal's process memory (`TerminalManager::memory`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSample {
    pub terminal_id: String,
    pub workspace_id: String,
    /// The shell and what it started. `None` when the terminal's processes cannot be known; see
    /// `ProcessTree::memory`.
    pub sample: Option<MemorySample>,
}

#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    #[error("no such terminal")]
    NotFound,
    #[error("the terminal has exited")]
    Exited,
    #[error("the size must be 2 to 1000 columns and 1 to 500 rows")]
    InvalidSize,
    #[error("the working directory does not exist: {}", .0.display())]
    CwdNotFound(PathBuf),
    #[error("the working directory is not a directory: {}", .0.display())]
    CwdNotDirectory(PathBuf),
    #[error("no such shell: {0}")]
    ShellNotFound(String),
    #[error("too many live terminals")]
    LimitReached,
    #[error("too many clients attached to the terminal")]
    TooManyAttachments,
    #[error("the input is larger than {MAX_INPUT_BYTES} bytes")]
    InputTooLarge,
    #[error("cannot start the terminal: {0}")]
    Spawn(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub type Result<T, E = TerminalError> = std::result::Result<T, E>;

/// The service's terminals. Cheap to clone; every clone is the same manager.
#[derive(Clone)]
pub struct TerminalManager {
    inner: Arc<Inner>,
}

impl fmt::Debug for TerminalManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TerminalManager")
            .field("live", &self.live_count())
            .finish_non_exhaustive()
    }
}

struct Inner {
    config: ManagerConfig,
    registry: Mutex<Registry>,
    /// Serializes creates that carry a request id, so a retry racing the first finds it.
    request_gate: Mutex<()>,
}

#[derive(Default)]
struct Registry {
    terminals: HashMap<String, Arc<Terminal>>,
    requests: HashMap<String, String>,
    /// Exited terminals, oldest first.
    exited: VecDeque<String>,
    /// Live terminals plus creates in progress.
    live: usize,
    next_seq: u64,
}

impl Registry {
    fn forget(&mut self, id: &str) -> Option<Arc<Terminal>> {
        let terminal = self.terminals.remove(id)?;
        if let Some(request) = &terminal.request_id
            && self.requests.get(request).is_some_and(|owner| owner == id)
        {
            self.requests.remove(request);
        }
        self.exited.retain(|exited| exited != id);
        Some(terminal)
    }
}

impl Inner {
    fn find(&self, id: &str) -> Result<Arc<Terminal>> {
        lock(&self.registry)
            .terminals
            .get(id)
            .cloned()
            .ok_or(TerminalError::NotFound)
    }

    /// Called by the waiter once the shell is reaped, just before the exit is recorded.
    fn retire(&self, id: &str) {
        let mut forgotten = Vec::new();
        {
            let mut registry = lock(&self.registry);
            registry.live = registry.live.saturating_sub(1);
            if registry.terminals.contains_key(id) {
                registry.exited.push_back(id.to_owned());
            }
            while registry.exited.len() > self.config.max_exited {
                let Some(oldest) = registry.exited.pop_front() else {
                    break;
                };
                forgotten.extend(registry.forget(&oldest));
            }
        }
        // Terminals drop outside the registry lock.
        drop(forgotten);
    }
}

impl TerminalManager {
    pub fn new(config: ManagerConfig) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                registry: Mutex::new(Registry::default()),
                request_gate: Mutex::new(()),
            }),
        }
    }

    /// Starts a terminal. Blocks while the program is spawned.
    pub fn create(&self, spec: CreateSpec) -> Result<TerminalInfo> {
        let _gate = spec
            .request_id
            .as_ref()
            .map(|_| lock(&self.inner.request_gate));
        if let Some(request) = &spec.request_id {
            let existing = lock(&self.inner.registry).requests.get(request).cloned();
            if let Some(terminal) = existing.and_then(|id| self.inner.find(&id).ok()) {
                return Ok(terminal.info());
            }
        }
        validate_size(spec.size)?;
        check_cwd(&spec.cwd)?;
        let (program, args) = self.resolve_shell(spec.shell.as_ref())?;
        let seq = {
            let mut registry = lock(&self.inner.registry);
            if registry.live >= self.inner.config.max_live {
                return Err(TerminalError::LimitReached);
            }
            registry.live += 1;
            registry.next_seq += 1;
            registry.next_seq
        };
        self.start(spec, program, args, seq).inspect_err(|_| {
            let mut registry = lock(&self.inner.registry);
            registry.live = registry.live.saturating_sub(1);
        })
    }

    fn resolve_shell(&self, shell: Option<&ShellChoice>) -> Result<(PathBuf, Vec<String>)> {
        let env = &self.inner.config.base_env;
        match shell {
            Some(choice) => resolve_program(&choice.program, env)
                .map(|program| (program, choice.args.clone()))
                .ok_or_else(|| TerminalError::ShellNotFound(choice.program.clone())),
            None => default_shell(env)
                .map(|program| (program, Vec::new()))
                .map_err(|error| TerminalError::ShellNotFound(error.to_string())),
        }
    }

    fn start(
        &self,
        spec: CreateSpec,
        program: PathBuf,
        args: Vec<String>,
        seq: u64,
    ) -> Result<TerminalInfo> {
        let config = &self.inner.config;
        let id = uuid::Uuid::now_v7().to_string();
        let env = build_environment(
            &config.base_env,
            &TerminalEnv {
                terminal_id: &id,
                workspace_id: &spec.workspace_id,
                service_version: &config.service_version,
                massorca_home: config.massorca_home.as_deref(),
                massorca_profile: config.massorca_profile.as_deref(),
                extra: &spec.env,
            },
        );
        let spawned = pty::spawn(&SpawnSpec {
            program: program.clone(),
            args,
            cwd: spec.cwd.clone(),
            env,
            size: spec.size,
        })
        .map_err(spawn_error)?;

        let terminal = Arc::new(Terminal {
            id: id.clone(),
            workspace_id: spec.workspace_id,
            shell: program.display().to_string(),
            cwd: spec.cwd.display().to_string(),
            created_at_ms: now_ms(),
            seq,
            title: spec.title,
            request_id: spec.request_id.clone(),
            holds_secrets: spec.holds_secrets,
            pid: spawned.master.pid(),
            master: Arc::new(spawned.master),
            tree: spawned.tree,
            core: Mutex::new(Core {
                screen: Screen::new(spec.size.cols, spec.size.rows),
                ring: ByteRing::new(RING_CAPACITY),
                size: spec.size,
                status: Status::Live,
                kill_reason: None,
                attachments: Vec::new(),
                query_match: 0,
                held: VecDeque::new(),
            }),
            changed: Condvar::new(),
            resize_gate: Mutex::new(()),
        });
        {
            // Registered before the waiter starts, so its `retire` finds the terminal.
            let mut registry = lock(&self.inner.registry);
            registry.terminals.insert(id.clone(), Arc::clone(&terminal));
            if let Some(request) = spec.request_id {
                registry.requests.insert(request, id.clone());
            }
        }

        let (reader_done, drained) = mpsc::channel::<()>();
        let reader = spawned.reader;
        let reading = Arc::clone(&terminal);
        let started = spawn_thread("terminal-reader", move || {
            read_output(&reading, reader, reader_done);
        })
        .and_then(|()| {
            let waiting = Arc::clone(&terminal);
            let manager = Arc::downgrade(&self.inner);
            let child = spawned.child;
            spawn_thread("terminal-waiter", move || {
                wait_for_exit(&waiting, child, &drained, &manager);
            })
        });
        if let Err(error) = started {
            terminal.tree.kill(None);
            close_off_thread(&terminal.master);
            drop(lock(&self.inner.registry).forget(&id));
            return Err(TerminalError::Spawn(error.to_string()));
        }
        Ok(terminal.info())
    }

    /// Every terminal, or a workspace's, by creation time.
    pub fn list(&self, workspace_id: Option<&str>) -> Vec<TerminalInfo> {
        let mut terminals: Vec<Arc<Terminal>> = lock(&self.inner.registry)
            .terminals
            .values()
            .filter(|terminal| workspace_id.is_none_or(|id| terminal.workspace_id == id))
            .cloned()
            .collect();
        terminals.sort_by_key(|terminal| terminal.seq);
        terminals.iter().map(|terminal| terminal.info()).collect()
    }

    pub fn get(&self, id: &str) -> Result<TerminalInfo> {
        Ok(self.inner.find(id)?.info())
    }

    /// Writes `bytes` to the terminal's input, in call order.
    pub fn input(&self, id: &str, bytes: &[u8]) -> Result<()> {
        if bytes.len() > MAX_INPUT_BYTES {
            return Err(TerminalError::InputTooLarge);
        }
        let terminal = self.inner.find(id)?;
        if terminal.is_exited() {
            return Err(TerminalError::Exited);
        }
        let reports = cursor_reports(bytes);
        let mut kept = None;
        if !reports.is_empty() {
            let mut core = lock(&terminal.core);
            let mut filtered = Vec::with_capacity(bytes.len());
            let mut from = 0;
            for report in reports {
                let Some(query) = core.held.pop_front() else {
                    break;
                };
                if query.answered {
                    filtered.extend_from_slice(&bytes[from..report.start]);
                    from = report.end;
                }
            }
            filtered.extend_from_slice(&bytes[from..]);
            kept = Some(filtered);
        }
        let bytes = kept.as_deref().unwrap_or(bytes);
        if bytes.is_empty() {
            return Ok(());
        }
        terminal.master.write_all(bytes).map_err(|error| {
            if error.kind() == io::ErrorKind::BrokenPipe || terminal.is_exited() {
                TerminalError::Exited
            } else {
                TerminalError::Io(error)
            }
        })
    }

    /// Sets the terminal's one size; every attachment hears `Resized`. The model and the
    /// attachments change first, then the PTY: output the program draws at the new size is
    /// parsed at it, and `Resized` is queued ahead of it (0034).
    pub fn resize(&self, id: &str, size: PtySize) -> Result<TerminalInfo> {
        validate_size(size)?;
        let terminal = self.inner.find(id)?;
        let _gate = lock(&terminal.resize_gate);
        let previous = {
            let mut core = lock(&terminal.core);
            if matches!(core.status, Status::Exited(_)) {
                return Err(TerminalError::Exited);
            }
            let previous = core.size;
            core.set_size(size);
            previous
        };
        // Outside the terminal lock: a closing pseudoconsole holds the master until the
        // reader, which needs that lock, drains it.
        if let Err(error) = terminal.master.resize(size) {
            let mut core = lock(&terminal.core);
            if matches!(core.status, Status::Exited(_)) {
                return Err(TerminalError::Exited);
            }
            // The PTY kept its old size, so the model goes back to it.
            core.set_size(previous);
            return Err(TerminalError::Io(io::Error::other(error.to_string())));
        }
        let core = lock(&terminal.core);
        Ok(terminal.info_locked(&core))
    }

    /// Ends the terminal's process tree (0031). Blocks until the exit is recorded or the
    /// grace and a short margin have passed.
    pub fn kill(&self, id: &str, force: bool) -> Result<TerminalInfo> {
        let terminal = self.inner.find(id)?;
        Ok(terminal.kill(
            force,
            ExitReason::Killed,
            self.inner.config.kill_grace,
            KILL_EXIT_MARGIN,
        ))
    }

    /// Kills the terminal if it is live, then forgets it.
    pub fn close(&self, id: &str) -> Result<TerminalInfo> {
        let terminal = self.inner.find(id)?;
        let info = terminal.kill(
            false,
            ExitReason::Killed,
            self.inner.config.kill_grace,
            KILL_EXIT_MARGIN,
        );
        let forgotten = lock(&self.inner.registry).forget(id);
        drop(forgotten);
        Ok(info)
    }

    /// Starts following the terminal's output: from `since` when the ring still holds it,
    /// else from a snapshot. Works on exited terminals: the snapshot, then `Exited`.
    /// At most `MAX_ATTACHMENTS` follow a terminal at once. `answers` says whether the client
    /// answers the cursor queries itself; only a client that does can hold one (0036).
    pub fn attach(
        &self,
        id: &str,
        since: Option<u64>,
        answers: QueryAnswers,
    ) -> Result<Attachment> {
        let terminal = self.inner.find(id)?;
        let shared = Arc::new(AttachShared::new(answers));
        let mut replies = Vec::new();
        // Under the lock: the copy, its offset, and the registration, so no chunk falls between.
        let frozen = {
            let mut core = lock(&terminal.core);
            let exited = matches!(core.status, Status::Exited(_));
            if !exited && core.attachments.len() >= MAX_ATTACHMENTS {
                return Err(TerminalError::TooManyAttachments);
            }
            let info = terminal.info_locked(&core);
            let resume = since.and_then(|since| Some((since, core.ring.read_from(since)?)));
            let mut queue = lock(&shared.queue);
            let frozen = match resume {
                Some((offset, bytes)) => {
                    queue.push_back(AttachItem::Attached {
                        info: info.clone(),
                        mode: AttachMode::Resume,
                        offset,
                    });
                    if !bytes.is_empty() {
                        queue.push_back(AttachItem::Output { offset, bytes });
                    }
                    None
                }
                None => {
                    let frozen = core.freeze_snapshot();
                    queue.push_back(AttachItem::Attached {
                        info: info.clone(),
                        mode: AttachMode::Snapshot,
                        offset: frozen.offset,
                    });
                    queue.snapshot_at = Some(Instant::now());
                    Some(frozen)
                }
            };
            if !exited {
                core.attachments.push(Arc::clone(&shared));
                answer_held(&mut core.held, &mut replies);
            } else {
                queue.push_back(AttachItem::Exited { info });
                queue.ended = true;
            }
            frozen
        };
        if !replies.is_empty() {
            // A failed reply means the terminal is closing; the waiter records that.
            let _ = terminal.master.write_all(&replies);
        }
        if let Some(frozen) = frozen {
            // Outside the lock. Output queued meanwhile follows the `Attached` item, so the
            // snapshot is inserted right after it.
            #[cfg(test)]
            note_lock_state(&terminal);
            let snapshot = frozen.into_item();
            lock(&shared.queue).insert_snapshot(snapshot);
        }
        Ok(Attachment::new(terminal, shared))
    }

    /// Kills every terminal not yet exited at once (reason `ServiceStopped`), bounded by the
    /// shutdown grace plus a short margin.
    pub fn shutdown(&self) {
        let terminals: Vec<Arc<Terminal>> = lock(&self.inner.registry)
            .terminals
            .values()
            .filter(|terminal| !terminal.is_exited())
            .cloned()
            .collect();
        let grace = self.inner.config.shutdown_grace;
        thread::scope(|scope| {
            for terminal in &terminals {
                let kill = move || {
                    terminal.kill(
                        false,
                        ExitReason::ServiceStopped,
                        grace,
                        SHUTDOWN_EXIT_MARGIN,
                    );
                };
                if thread::Builder::new()
                    .name("terminal-shutdown".to_owned())
                    .spawn_scoped(scope, kill)
                    .is_err()
                {
                    terminal.kill(
                        true,
                        ExitReason::ServiceStopped,
                        grace,
                        SHUTDOWN_EXIT_MARGIN,
                    );
                }
            }
        });
    }

    /// Test-only seam, not part of the API: the terminal's waiter behaves as if its wait
    /// had failed, while the real wait goes on, so a later reap still records the exit.
    #[doc(hidden)]
    pub fn simulate_failed_wait(&self, id: &str) -> Result<()> {
        let terminal = self.inner.find(id)?;
        terminal.mark_unverifiable(&io::Error::other("simulated wait failure"));
        Ok(())
    }

    /// Live terminals, counting creates in progress.
    pub fn live_count(&self) -> usize {
        lock(&self.inner.registry).live
    }

    /// The process memory of every terminal not yet exited, by creation time, for host memory (decision 0081). A
    /// terminal whose wait failed (`Unverifiable`) is included. On Unix the process table is taken once for all
    /// terminals; see `ProcessTree::memory` for what each platform reads. Blocking: it makes system calls.
    pub fn memory(&self) -> Vec<TerminalSample> {
        let table = if cfg!(unix) {
            massorca_host_stats::processes()
        } else {
            Vec::new()
        };
        let mut terminals: Vec<Arc<Terminal>> = lock(&self.inner.registry)
            .terminals
            .values()
            .cloned()
            .collect();
        terminals.sort_by_key(|terminal| terminal.seq);
        terminals
            .iter()
            .filter(|terminal| !terminal.is_exited())
            .map(|terminal| TerminalSample {
                terminal_id: terminal.id.clone(),
                workspace_id: terminal.workspace_id.clone(),
                sample: terminal.tree.memory(&table),
            })
            .collect()
    }
}

fn validate_size(size: PtySize) -> Result<()> {
    if (2..=1000).contains(&size.cols) && (1..=500).contains(&size.rows) {
        Ok(())
    } else {
        Err(TerminalError::InvalidSize)
    }
}

fn check_cwd(cwd: &Path) -> Result<()> {
    match std::fs::metadata(cwd) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(TerminalError::CwdNotDirectory(cwd.to_path_buf())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(TerminalError::CwdNotFound(cwd.to_path_buf()))
        }
        Err(error) => Err(TerminalError::Io(error)),
    }
}

fn spawn_error(error: PtyError) -> TerminalError {
    match error {
        PtyError::ProgramNotAbsolute(program) => {
            TerminalError::ShellNotFound(program.display().to_string())
        }
        PtyError::CwdNotFound(cwd) => TerminalError::CwdNotFound(cwd),
        PtyError::Io(error) => TerminalError::Io(error),
        PtyError::Open(message) | PtyError::Spawn(message) | PtyError::Resize(message) => {
            TerminalError::Spawn(message)
        }
    }
}

fn spawn_thread(name: &str, run: impl FnOnce() + Send + 'static) -> io::Result<()> {
    thread::Builder::new()
        .name(name.to_owned())
        .spawn(run)
        .map(drop)
}

/// `ClosePseudoConsole` can block until the output is drained (before Windows 11 24H2),
/// so it never runs on the reader thread or under a lock.
fn close_off_thread(master: &Arc<PtyMaster>) {
    let closing = Arc::clone(master);
    if spawn_thread("terminal-close", move || closing.close()).is_err() {
        master.close();
    }
}

#[derive(Debug)]
enum Status {
    Live,
    Exited(ExitRecord),
    Unverifiable,
}

struct Core {
    screen: Screen,
    ring: ByteRing,
    size: PtySize,
    status: Status,
    /// Set by the first kill, read when the exit is recorded.
    kill_reason: Option<ExitReason>,
    attachments: Vec<Arc<AttachShared>>,
    /// Bytes of a cursor query matched at the end of the previous read.
    query_match: usize,
    /// Cursor queries that arrived while an emulator was attached, oldest first. A client's
    /// reply takes the oldest off. The service answers them itself when a client attaches
    /// (its snapshot cannot show the query) and when every emulator that received one has
    /// detached, so a stalled emulator cannot hold the shell past either. A query no attached
    /// emulator can answer is answered at once (0036).
    held: VecDeque<HeldQuery>,
}

/// A cursor query an attached client may still answer.
struct HeldQuery {
    reply: Vec<u8>,
    /// The service already wrote `reply`; a client's late reply to it is dropped.
    answered: bool,
    /// The emulators attached when the query arrived: the only clients that can reply to it.
    witnesses: Vec<usize>,
}

impl Core {
    /// Sets the model's size and tells every attachment. Never blocks.
    fn set_size(&mut self, size: PtySize) {
        self.screen.resize(size.cols, size.rows);
        self.size = size;
        for attachment in &self.attachments {
            attachment.push(AttachItem::Resized {
                cols: size.cols,
                rows: size.rows,
            });
        }
    }

    /// Copies what a snapshot needs, under the lock; serialize it after the lock is released.
    fn freeze_snapshot(&self) -> FrozenSnapshot {
        FrozenSnapshot {
            offset: self.ring.end_offset(),
            cols: self.size.cols,
            rows: self.size.rows,
            screen: self.screen.freeze(),
        }
    }

    /// Applies one PTY read to the screen, the ring and the attachments. Returns the
    /// replies to cursor queries no attached emulator can answer; they are written after
    /// the lock is released.
    fn ingest(&mut self, chunk: &[u8]) -> Vec<u8> {
        let mut replies = Vec::new();
        let mut fed = 0;
        for (index, &byte) in chunk.iter().enumerate() {
            self.query_match = if byte == CURSOR_QUERY[self.query_match] {
                self.query_match + 1
            } else {
                usize::from(byte == CURSOR_QUERY[0])
            };
            if self.query_match == CURSOR_QUERY.len() {
                self.query_match = 0;
                // The answer is the cursor as of the query, not as of the end of the read.
                self.screen.process(&chunk[fed..=index]);
                fed = index + 1;
                let (row, col) = self.screen.cursor_position();
                let reply = format!("\x1b[{};{}R", u32::from(row) + 1, u32::from(col) + 1);
                // Only an emulator attached now can reply, so only it is a witness (0036).
                let witnesses: Vec<usize> = self
                    .attachments
                    .iter()
                    .filter(|attachment| attachment.answers == QueryAnswers::Client)
                    .map(attachment_key)
                    .collect();
                if witnesses.is_empty() {
                    replies.extend_from_slice(reply.as_bytes());
                } else {
                    self.held.push_back(HeldQuery {
                        reply: reply.into_bytes(),
                        answered: false,
                        witnesses,
                    });
                }
            }
        }
        self.screen.process(&chunk[fed..]);
        let offset = self.ring.end_offset();
        self.ring.push(chunk);
        for attachment in &self.attachments {
            attachment.push(AttachItem::Output {
                offset,
                bytes: chunk.to_vec(),
            });
        }
        replies
    }
}

/// A snapshot's state, copied under the terminal lock and serialized after it is released.
struct FrozenSnapshot {
    offset: u64,
    cols: u16,
    rows: u16,
    screen: FrozenScreen,
}

#[cfg(test)]
thread_local! {
    /// Test seam: whether the terminal lock was free when a snapshot started to serialize.
    static LOCK_FREE_AT_SERIALIZE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

impl FrozenSnapshot {
    /// Serializes the copy. The caller must not hold the terminal lock.
    fn into_item(self) -> AttachItem {
        AttachItem::Snapshot {
            offset: self.offset,
            cols: self.cols,
            rows: self.rows,
            bytes: self.screen.serialize(),
        }
    }
}

/// Records whether the terminal lock is free where a snapshot starts to serialize.
#[cfg(test)]
fn note_lock_state(terminal: &Terminal) {
    LOCK_FREE_AT_SERIALIZE.with(|free| free.set(Some(terminal.core.try_lock().is_ok())));
}

pub(crate) struct Terminal {
    id: String,
    workspace_id: String,
    shell: String,
    cwd: String,
    created_at_ms: i64,
    /// Creation order, for `list`.
    seq: u64,
    title: Option<String>,
    request_id: Option<String>,
    holds_secrets: bool,
    pid: Option<u32>,
    master: Arc<PtyMaster>,
    tree: ProcessTree,
    core: Mutex<Core>,
    /// Signalled when the exit is recorded, and by a kill to wake a waiter retrying its wait.
    changed: Condvar,
    /// Keeps the PTY and the screen sizes in the same order.
    resize_gate: Mutex<()>,
}

impl Terminal {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    /// Only a confirmed reap counts: `Unverifiable` is not exited.
    fn is_exited(&self) -> bool {
        matches!(lock(&self.core).status, Status::Exited(_))
    }

    fn info(&self) -> TerminalInfo {
        self.info_locked(&lock(&self.core))
    }

    fn info_locked(&self, core: &Core) -> TerminalInfo {
        let (state, exit) = match &core.status {
            Status::Live => (TerminalState::Live, None),
            Status::Exited(exit) => (TerminalState::Exited, Some(exit.clone())),
            Status::Unverifiable => (TerminalState::Unverifiable, None),
        };
        TerminalInfo {
            id: self.id.clone(),
            workspace_id: self.workspace_id.clone(),
            state,
            title: self
                .title
                .clone()
                .or_else(|| core.screen.title().map(str::to_owned)),
            cols: core.size.cols,
            rows: core.size.rows,
            pid: self.pid,
            shell: self.shell.clone(),
            cwd: self.cwd.clone(),
            created_at_ms: self.created_at_ms,
            exit,
            holds_secrets: self.holds_secrets,
        }
    }

    /// Graceful: hang up and close the terminal, wait up to `grace`, then kill the tree.
    /// Force: kill the tree at once. Either way, what is left of the tree is killed.
    /// An exited terminal still gets the tree kill: it only reaches what was recorded.
    fn kill(
        &self,
        force: bool,
        reason: ExitReason,
        grace: Duration,
        margin: Duration,
    ) -> TerminalInfo {
        let exited = {
            let mut core = lock(&self.core);
            let exited = matches!(core.status, Status::Exited(_));
            if !exited {
                core.kill_reason.get_or_insert(reason);
            }
            exited
        };
        if exited {
            self.tree.kill(self.master.foreground_process_group());
            return self.info();
        }
        if !force {
            self.tree.hangup(self.master.foreground_process_group());
            close_off_thread(&self.master);
            self.wait_for_exit(grace);
        }
        self.tree.kill(self.master.foreground_process_group());
        // A waiter retrying a failed wait tries again now.
        self.changed.notify_all();
        self.wait_for_exit(margin)
    }

    /// Reaps the shell once it has exited, through the tree (0031).
    fn reap(&self, child: &mut PtyChild) -> io::Result<ExitInfo> {
        self.tree
            .reap_shell(|| self.master.foreground_process_group(), || child.wait())
    }

    fn wait_for_exit(&self, timeout: Duration) -> TerminalInfo {
        let deadline = Instant::now() + timeout;
        let mut core = lock(&self.core);
        while !matches!(core.status, Status::Exited(_)) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            core = self
                .changed
                .wait_timeout(core, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        self.info_locked(&core)
    }

    /// A failed wait: the process may still run, so attachments stay open.
    fn mark_unverifiable(&self, error: &io::Error) {
        tracing::warn!(terminal = %self.id, error = %error, "a terminal's exit could not be read");
        let mut core = lock(&self.core);
        if matches!(core.status, Status::Live) {
            core.status = Status::Unverifiable;
        }
    }

    /// Waits up to `pause` before the waiter retries a failed wait; a kill cuts it short.
    fn pause_before_retry(&self, pause: Duration) {
        let core = lock(&self.core);
        drop(
            self.changed
                .wait_timeout(core, pause)
                .unwrap_or_else(PoisonError::into_inner),
        );
    }

    /// The confirmed reap: the only way a terminal becomes `Exited`.
    fn record_exit(&self, exit: ExitInfo) {
        let mut core = lock(&self.core);
        core.status = Status::Exited(ExitRecord {
            code: exit.code,
            signal: exit.signal,
            reason: core.kill_reason.unwrap_or(ExitReason::Exited),
            at_ms: now_ms(),
        });
        let info = self.info_locked(&core);
        for attachment in core.attachments.drain(..) {
            attachment.push(AttachItem::Exited { info: info.clone() });
        }
        self.changed.notify_all();
    }

    /// The recovery of a lagging attachment: a fresh snapshot, atomically with its queue.
    pub(crate) fn resnapshot(&self, shared: &AttachShared) -> AttachItem {
        let frozen = {
            let core = lock(&self.core);
            let frozen = core.freeze_snapshot();
            let mut queue = lock(&shared.queue);
            queue.clear();
            queue.lagging = false;
            queue.snapshot_at = Some(Instant::now());
            if matches!(core.status, Status::Exited(_)) {
                queue.push_back(AttachItem::Exited {
                    info: self.info_locked(&core),
                });
                queue.ended = true;
            }
            frozen
        };
        #[cfg(test)]
        note_lock_state(self);
        // Returned straight to the reader of this attachment, so nothing queues ahead of it.
        frozen.into_item()
    }

    pub(crate) fn detach(&self, shared: &Arc<AttachShared>) {
        let mut replies = Vec::new();
        {
            let mut core = lock(&self.core);
            core.attachments
                .retain(|attachment| !Arc::ptr_eq(attachment, shared));
            let leaving = attachment_key(shared);
            // A query no attached client received can only be answered by the service, and
            // once answered, no late reply to it can come.
            core.held.retain_mut(|query| {
                query.witnesses.retain(|witness| *witness != leaving);
                if !query.witnesses.is_empty() {
                    return true;
                }
                if !query.answered {
                    replies.extend_from_slice(&query.reply);
                }
                false
            });
        }
        if !replies.is_empty() {
            // A failed reply means the terminal is closing; the waiter records that.
            let _ = self.master.write_all(&replies);
        }
    }
}

/// Identity of an attachment while it is registered.
fn attachment_key(attachment: &Arc<AttachShared>) -> usize {
    Arc::as_ptr(attachment).addr()
}

/// Writes the reply of every held query not yet answered into `replies`, marking it answered.
fn answer_held(held: &mut VecDeque<HeldQuery>, replies: &mut Vec<u8>) {
    for query in held.iter_mut().filter(|query| !query.answered) {
        replies.extend_from_slice(&query.reply);
        query.answered = true;
    }
}

/// Byte ranges of the cursor position reports (`ESC [ row ; col R`) in a client's input.
fn cursor_reports(bytes: &[u8]) -> Vec<std::ops::Range<usize>> {
    let mut reports = Vec::new();
    let mut at = 0;
    while let Some(found) = bytes[at..].windows(2).position(|pair| pair == b"\x1b[") {
        let start = at + found;
        let body_start = start + 2;
        let body = bytes[body_start..]
            .iter()
            .take_while(|byte| byte.is_ascii_digit() || **byte == b';')
            .count();
        let body_end = body_start + body;
        let separators = bytes[body_start..body_end]
            .iter()
            .filter(|byte| **byte == b';')
            .count();
        if bytes.get(body_end) == Some(&b'R') && separators == 1 {
            reports.push(start..body_end + 1);
        }
        at = body_start;
    }
    reports
}

fn read_output(terminal: &Terminal, mut reader: PtyReader, _done: mpsc::Sender<()>) {
    let mut buffer = vec![0_u8; READ_CHUNK];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            // On Windows a read fails once the pseudoconsole is closed: end of output.
            Err(_) => break,
        };
        let replies = lock(&terminal.core).ingest(&buffer[..read]);
        if !replies.is_empty() {
            // A failed reply means the terminal is closing; the waiter records that.
            let _ = terminal.master.write_all(&replies);
        }
    }
}

fn wait_for_exit(
    terminal: &Terminal,
    mut child: PtyChild,
    drained: &mpsc::Receiver<()>,
    manager: &Weak<Inner>,
) {
    let mut pause = WAIT_RETRY_FIRST;
    let exit = loop {
        match terminal.reap(&mut child) {
            Ok(exit) => break exit,
            Err(error) => {
                terminal.mark_unverifiable(&error);
                terminal.pause_before_retry(pause);
                pause = (pause * 2).min(WAIT_RETRY_MAX);
            }
        }
    };
    // Close first: on Windows that is what ends the output. The channel disconnects when
    // the reader ends, so every attachment has all the output before `Exited`.
    close_off_thread(&terminal.master);
    let _ = drained.recv_timeout(DRAIN_LIMIT);
    // Retired first: whoever `record_exit` wakes (a kill) already sees the live count lowered.
    if let Some(manager) = manager.upgrade() {
        manager.retire(&terminal.id);
    }
    terminal.record_exit(exit);
}

#[cfg(test)]
mod tests {
    use super::TerminalManager;
    use crate::attach::Attachment;

    fn is_send<T: Send>() {}
    fn is_send_sync<T: Send + Sync>() {}

    #[test]
    fn finds_only_cursor_position_reports() {
        use super::cursor_reports;
        assert_eq!(cursor_reports(b"\x1b[12;40R"), vec![0..8]);
        assert_eq!(cursor_reports(b"a\x1b[1;1Rb\x1b[3;4R"), vec![1..7, 8..14]);
        // Arrow keys, a bare `R` and a report without a column are not answers.
        assert!(cursor_reports(b"\x1b[A\x1b[1;5A R \x1b[5R").is_empty());
    }

    #[test]
    fn handles_cross_threads_and_tasks() {
        is_send_sync::<TerminalManager>();
        is_send::<Attachment>();
    }

    #[tokio::test]
    async fn the_next_future_is_send() {
        fn send_future<F: std::future::Future + Send>(_: &F) {}
        let manager = TerminalManager::new(super::ManagerConfig::new(
            crate::shell::EnvSnapshot::default(),
            "0.0.0-test",
        ));
        if let Ok(mut attachment) = manager.attach("missing", None, super::QueryAnswers::Service) {
            send_future(&attachment.next());
        }
    }

    #[test]
    fn a_resize_sets_the_screen_model_to_the_new_size() {
        use std::collections::BTreeMap;

        use super::{CreateSpec, ShellChoice};
        use crate::pty::PtySize;

        let manager = TerminalManager::new(super::ManagerConfig::new(
            crate::shell::EnvSnapshot::current(),
            "0.0.0-test",
        ));
        let shell = if cfg!(windows) {
            ShellChoice {
                program: "cmd.exe".to_owned(),
                args: vec!["/d".to_owned()],
            }
        } else {
            ShellChoice {
                program: "sh".to_owned(),
                args: Vec::new(),
            }
        };
        let info = manager
            .create(CreateSpec {
                workspace_id: "ws".to_owned(),
                cwd: std::env::temp_dir(),
                shell: Some(shell),
                env: BTreeMap::new(),
                size: PtySize { cols: 80, rows: 24 },
                title: None,
                request_id: None,
                holds_secrets: false,
            })
            .expect("create the terminal");
        let size = PtySize {
            cols: 100,
            rows: 30,
        };

        manager.resize(&info.id, size).expect("resize");

        let terminal = super::lock(&manager.inner.registry)
            .terminals
            .get(&info.id)
            .cloned()
            .expect("the terminal is registered");
        let core = super::lock(&terminal.core);
        assert_eq!(core.size, size);
        assert_eq!(core.screen.size(), (100, 30));
        drop(core);
        manager.kill(&info.id, true).expect("kill");
    }

    /// A snapshot is serialized only once the terminal lock is free, so the PTY reader is never
    /// held up by building it (decision 0034). The seam records the lock state at that point.
    #[test]
    fn a_snapshot_is_serialized_after_the_terminal_lock_is_released() {
        use std::cell::Cell;
        use std::collections::BTreeMap;

        use super::{CreateSpec, LOCK_FREE_AT_SERIALIZE, ShellChoice};
        use crate::pty::PtySize;

        let manager = TerminalManager::new(super::ManagerConfig::new(
            crate::shell::EnvSnapshot::current(),
            "0.0.0-test",
        ));
        let shell = if cfg!(windows) {
            ShellChoice {
                program: "cmd.exe".to_owned(),
                args: vec!["/d".to_owned()],
            }
        } else {
            ShellChoice {
                program: "sh".to_owned(),
                args: Vec::new(),
            }
        };
        let info = manager
            .create(CreateSpec {
                workspace_id: "ws".to_owned(),
                cwd: std::env::temp_dir(),
                shell: Some(shell),
                env: BTreeMap::new(),
                size: PtySize { cols: 80, rows: 24 },
                title: None,
                request_id: None,
                holds_secrets: false,
            })
            .expect("create the terminal");

        LOCK_FREE_AT_SERIALIZE.with(|free| free.set(None));
        let attachment = manager
            .attach(&info.id, None, super::QueryAnswers::Service)
            .expect("attach");

        assert_eq!(LOCK_FREE_AT_SERIALIZE.with(Cell::get), Some(true));
        drop(attachment);
        manager.kill(&info.id, true).expect("kill");
    }

    /// A shell that exits while a background process still holds its terminal: the reader
    /// and the waiter must end, which releases the terminal's buffers (decision 0034).
    #[test]
    fn a_shell_that_exits_releases_its_reader_and_waiter() {
        use std::collections::BTreeMap;
        use std::sync::Arc;
        use std::time::{Duration, Instant};

        use super::{CreateSpec, ShellChoice, TerminalState};
        use crate::pty::PtySize;

        let manager = TerminalManager::new(super::ManagerConfig::new(
            crate::shell::EnvSnapshot::current(),
            "0.0.0-test",
        ));
        let (program, args): (&str, Vec<&str>) = if cfg!(windows) {
            (
                "cmd.exe",
                vec!["/d", "/c", "start /b ping -n 120 127.0.0.1 >nul & exit 0"],
            )
        } else {
            ("bash", vec!["-c", "set -m; sleep 120 & exit 0"])
        };
        let info = manager
            .create(CreateSpec {
                workspace_id: "ws".to_owned(),
                cwd: std::env::temp_dir(),
                shell: Some(ShellChoice {
                    program: program.to_owned(),
                    args: args.into_iter().map(str::to_owned).collect(),
                }),
                env: BTreeMap::new(),
                size: PtySize { cols: 80, rows: 24 },
                title: None,
                request_id: None,
                holds_secrets: false,
            })
            .expect("create the terminal");
        let terminal = super::lock(&manager.inner.registry)
            .terminals
            .get(&info.id)
            .cloned()
            .expect("the terminal is registered");

        let deadline = Instant::now() + Duration::from_secs(10);
        while manager.get(&info.id).expect("listed").state != TerminalState::Exited {
            assert!(Instant::now() < deadline, "the shell did not exit");
            std::thread::sleep(Duration::from_millis(20));
        }
        // Only the registry and this test hold the terminal once the reader and waiter end.
        while Arc::strong_count(&terminal) != 2 {
            assert!(
                Instant::now() < deadline,
                "the reader or the waiter still holds the terminal"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
