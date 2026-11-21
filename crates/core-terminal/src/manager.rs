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

use crate::attach::{AttachItem, AttachMode, AttachShared, Attachment};
use crate::process_tree::ProcessTree;
use crate::pty::{self, ExitInfo, PtyChild, PtyError, PtyMaster, PtyReader, PtySize, SpawnSpec};
use crate::ring::{ByteRing, RING_CAPACITY};
use crate::screen::Screen;
use crate::shell::{EnvSnapshot, TerminalEnv, build_environment, default_shell, resolve_program};

/// Largest input accepted per call (0033).
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
const READ_CHUNK: usize = 64 * 1024;
/// How long the reader may drain after the program exits (0030).
const DRAIN_LIMIT: Duration = Duration::from_millis(500);
/// How long a kill waits for the exit after the tree is killed.
const EXIT_MARGIN: Duration = Duration::from_secs(2);
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
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalState {
    Live,
    /// The exit could not be established; never treated as death.
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

    /// Called by the waiter once the exit is recorded.
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
            terminal.tree.kill();
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
        if !terminal.is_live() {
            return Err(TerminalError::Exited);
        }
        terminal.master.write_all(bytes).map_err(|error| {
            if error.kind() == io::ErrorKind::BrokenPipe || !terminal.is_live() {
                TerminalError::Exited
            } else {
                TerminalError::Io(error)
            }
        })
    }

    /// Sets the terminal's one size; every attachment hears `Resized`.
    pub fn resize(&self, id: &str, size: PtySize) -> Result<TerminalInfo> {
        validate_size(size)?;
        let terminal = self.inner.find(id)?;
        let _gate = lock(&terminal.resize_gate);
        if !terminal.is_live() {
            return Err(TerminalError::Exited);
        }
        // Outside the terminal lock: a closing pseudoconsole holds the master until the
        // reader, which needs that lock, drains it.
        terminal.master.resize(size).map_err(|error| {
            if terminal.is_live() {
                TerminalError::Io(io::Error::other(error.to_string()))
            } else {
                TerminalError::Exited
            }
        })?;
        let mut core = lock(&terminal.core);
        core.screen.resize(size.cols, size.rows);
        core.size = size;
        for attachment in &core.attachments {
            attachment.push(AttachItem::Resized {
                cols: size.cols,
                rows: size.rows,
            });
        }
        Ok(terminal.info_locked(&core))
    }

    /// Ends the terminal's process tree (0031). Blocks until the exit is recorded or the
    /// grace and a short margin have passed.
    pub fn kill(&self, id: &str, force: bool) -> Result<TerminalInfo> {
        let terminal = self.inner.find(id)?;
        Ok(terminal.kill(force, ExitReason::Killed, self.inner.config.kill_grace))
    }

    /// Kills the terminal if it is live, then forgets it.
    pub fn close(&self, id: &str) -> Result<TerminalInfo> {
        let terminal = self.inner.find(id)?;
        let info = terminal.kill(false, ExitReason::Killed, self.inner.config.kill_grace);
        let forgotten = lock(&self.inner.registry).forget(id);
        drop(forgotten);
        Ok(info)
    }

    /// Starts following the terminal's output: from `since` when the ring still holds it,
    /// else from a snapshot. Works on exited terminals: the snapshot, then `Exited`.
    pub fn attach(&self, id: &str, since: Option<u64>) -> Result<Attachment> {
        let terminal = self.inner.find(id)?;
        let shared = Arc::new(AttachShared::default());
        {
            let mut core = lock(&terminal.core);
            let info = terminal.info_locked(&core);
            let resume = since.and_then(|since| Some((since, core.ring.read_from(since)?)));
            let mut queue = lock(&shared.queue);
            match resume {
                Some((offset, bytes)) => {
                    queue.items.push_back(AttachItem::Attached {
                        info: info.clone(),
                        mode: AttachMode::Resume,
                        offset,
                    });
                    if !bytes.is_empty() {
                        queue.items.push_back(AttachItem::Output { offset, bytes });
                    }
                }
                None => {
                    let (offset, snapshot) = core.snapshot();
                    queue.items.push_back(AttachItem::Attached {
                        info: info.clone(),
                        mode: AttachMode::Snapshot,
                        offset,
                    });
                    queue.items.push_back(snapshot);
                }
            }
            if matches!(core.status, Status::Live) {
                core.attachments.push(Arc::clone(&shared));
            } else {
                queue.items.push_back(AttachItem::Exited { info });
                queue.ended = true;
            }
        }
        Ok(Attachment::new(terminal, shared))
    }

    /// Kills every live terminal at once (reason `ServiceStopped`), bounded by the shutdown
    /// grace plus a short margin.
    pub fn shutdown(&self) {
        let terminals: Vec<Arc<Terminal>> = lock(&self.inner.registry)
            .terminals
            .values()
            .filter(|terminal| terminal.is_live())
            .cloned()
            .collect();
        let grace = self.inner.config.shutdown_grace;
        thread::scope(|scope| {
            for terminal in &terminals {
                let kill = move || {
                    terminal.kill(false, ExitReason::ServiceStopped, grace);
                };
                if thread::Builder::new()
                    .name("terminal-shutdown".to_owned())
                    .spawn_scoped(scope, kill)
                    .is_err()
                {
                    terminal.kill(true, ExitReason::ServiceStopped, grace);
                }
            }
        });
    }

    /// Live terminals, counting creates in progress.
    pub fn live_count(&self) -> usize {
        lock(&self.inner.registry).live
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
}

impl Core {
    /// The snapshot item and the offset output continues from.
    fn snapshot(&mut self) -> (u64, AttachItem) {
        let offset = self.ring.end_offset();
        let item = AttachItem::Snapshot {
            offset,
            cols: self.size.cols,
            rows: self.size.rows,
            bytes: self.screen.snapshot(),
        };
        (offset, item)
    }

    /// Applies one PTY read to the screen, the ring and the attachments. Returns the
    /// replies to cursor queries nobody attached can answer; they are written after the
    /// lock is released.
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
                if self.attachments.is_empty() {
                    let (row, col) = self.screen.cursor_position();
                    replies.extend_from_slice(
                        format!("\x1b[{};{}R", u32::from(row) + 1, u32::from(col) + 1).as_bytes(),
                    );
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
    pid: Option<u32>,
    master: Arc<PtyMaster>,
    tree: ProcessTree,
    core: Mutex<Core>,
    /// Signalled when the exit is recorded.
    changed: Condvar,
    /// Keeps the PTY and the screen sizes in the same order.
    resize_gate: Mutex<()>,
}

impl Terminal {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    fn is_live(&self) -> bool {
        matches!(lock(&self.core).status, Status::Live)
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
        }
    }

    /// Graceful: hang up and close the terminal, wait up to `grace`, then kill the tree.
    /// Force: kill the tree at once. Either way, what is left of the tree is killed.
    fn kill(&self, force: bool, reason: ExitReason, grace: Duration) -> TerminalInfo {
        {
            let mut core = lock(&self.core);
            if !matches!(core.status, Status::Live) {
                return self.info_locked(&core);
            }
            core.kill_reason.get_or_insert(reason);
        }
        if !force {
            self.tree.hangup();
            close_off_thread(&self.master);
            self.wait_for_exit(grace);
        }
        self.tree.kill();
        self.wait_for_exit(EXIT_MARGIN)
    }

    fn wait_for_exit(&self, timeout: Duration) -> TerminalInfo {
        let deadline = Instant::now() + timeout;
        let mut core = lock(&self.core);
        while matches!(core.status, Status::Live) {
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

    fn record_exit(&self, result: io::Result<ExitInfo>) {
        let mut core = lock(&self.core);
        core.status = match result {
            Ok(exit) => Status::Exited(ExitRecord {
                code: exit.code,
                signal: exit.signal,
                reason: core.kill_reason.unwrap_or(ExitReason::Exited),
                at_ms: now_ms(),
            }),
            Err(error) => {
                tracing::warn!(terminal = %self.id, error = %error, "a terminal's exit could not be read");
                Status::Unverifiable
            }
        };
        let info = self.info_locked(&core);
        for attachment in core.attachments.drain(..) {
            attachment.push(AttachItem::Exited { info: info.clone() });
        }
        self.changed.notify_all();
    }

    /// The recovery of a lagging attachment: a fresh snapshot, atomically with its queue.
    pub(crate) fn resnapshot(&self, shared: &AttachShared) -> AttachItem {
        let mut core = lock(&self.core);
        let (_, snapshot) = core.snapshot();
        let mut queue = lock(&shared.queue);
        queue.items.clear();
        queue.lagging = false;
        if !matches!(core.status, Status::Live) {
            queue.items.push_back(AttachItem::Exited {
                info: self.info_locked(&core),
            });
            queue.ended = true;
        }
        snapshot
    }

    pub(crate) fn detach(&self, shared: &Arc<AttachShared>) {
        lock(&self.core)
            .attachments
            .retain(|attachment| !Arc::ptr_eq(attachment, shared));
    }
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
    let result = child.wait();
    // Disconnects when the reader ends; output still arriving after this is not awaited.
    let _ = drained.recv_timeout(DRAIN_LIMIT);
    close_off_thread(&terminal.master);
    terminal.record_exit(result);
    if let Some(manager) = manager.upgrade() {
        manager.retire(&terminal.id);
    }
}

#[cfg(test)]
mod tests {
    use super::TerminalManager;
    use crate::attach::Attachment;

    fn is_send<T: Send>() {}
    fn is_send_sync<T: Send + Sync>() {}

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
        if let Ok(mut attachment) = manager.attach("missing", None) {
            send_future(&attachment.next());
        }
    }
}
