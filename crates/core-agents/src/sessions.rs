//! Agent sessions (decision 0051): the service object that starts runs in terminals, keeps session rows, reads each
//! session's execution from its terminal, and keeps each session's status (0052, 0053).
//!
//! Calls block (store, process spawn), so the service runs them on its blocking pool.
//!
//! Every status change (a start, a hook report, an exit, a forget) is applied under one lock, written to the session
//! row, and handed to the change listener in that same order.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, Weak};
use std::thread;
use std::time::{Duration, Instant};

use massorca_core_store::{
    AgentSessionRow, RunExit, RunExitReason, Store, StoreError, StoredActivity, new_id, now_ms,
};
use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::{
    CreateSpec, ExitReason as CoreExitReason, ExitRecord, ShellChoice, TerminalError,
    TerminalManager, TerminalState as CoreState,
};
use massorca_core_workspace::{DomainError, Workspaces};
use massorca_protocol::methods::agents::{
    Activity, AgentHookParams, AgentKinds, AgentSession, AgentStartParams, Execution,
    MAX_HOOK_PAYLOAD_BYTES, PromptReceipt, ReceiptKind,
};
use massorca_protocol::methods::terminals::{
    DEFAULT_COLS, DEFAULT_ROWS, ExitReason, TerminalExit, TerminalState,
};
use massorca_protocol::{ErrorBody, codes};
use serde_json::{Value, json};

use crate::catalog::Catalog;
use crate::kind::{KindDecl, PasteMode, PromptMode, StatusMechanism};
use crate::launch::{self, LaunchError, LaunchInput, LaunchPlan, Run};
use crate::paths;
use crate::prompt;
use crate::resolve::{SearchEnv, resolve_first};
use crate::status::{self, HookRules};

/// How often the terminals of live runs are checked for an exit (0052).
pub const SWEEP_INTERVAL: Duration = Duration::from_millis(200);

/// How long a first prompt waits for its run's first `idle` before it is dropped (0054). The bound exists because an
/// agent that never reports idle would otherwise hold the text for as long as its run lives.
pub const FIRST_PROMPT_WAIT: Duration = Duration::from_secs(120);

/// One applied status change, in the order the store applied it.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// The session's full record after the change.
    Updated(Box<AgentSession>),
    /// The session was forgotten; its id.
    Forgotten(String),
}

/// Receives every [`Change`]. Called under the status lock, so it must not block or call back into [`Agents`].
pub type Listener = Box<dyn Fn(Change) + Send + Sync>;

/// Shortest id prefix a session selector may be.
pub const MIN_SELECTOR_PREFIX: usize = 8;

/// A refused or failed agent call: a stable protocol code, a message for humans, optional data.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentError {
    pub code: &'static str,
    pub message: String,
    pub data: Option<Value>,
}

impl AgentError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AgentError {}

impl From<AgentError> for ErrorBody {
    fn from(error: AgentError) -> Self {
        Self {
            code: error.code.to_owned(),
            message: error.message,
            data: error.data,
        }
    }
}

impl From<DomainError> for AgentError {
    fn from(error: DomainError) -> Self {
        Self::new(error.code(), error.message())
    }
}

impl From<StoreError> for AgentError {
    fn from(error: StoreError) -> Self {
        let code = match error {
            StoreError::NewerSchema { .. } => codes::STORE_NEWER_SCHEMA,
            _ => codes::INTERNAL,
        };
        Self::new(code, error.to_string())
    }
}

impl From<TerminalError> for AgentError {
    fn from(error: TerminalError) -> Self {
        let code = match &error {
            TerminalError::NotFound => codes::TERMINAL_NOT_FOUND,
            TerminalError::Exited => codes::TERMINAL_EXITED,
            TerminalError::InvalidSize => codes::TERMINAL_INVALID_SIZE,
            TerminalError::CwdNotFound(_) => codes::PATH_NOT_FOUND,
            TerminalError::CwdNotDirectory(_) => codes::PATH_NOT_DIRECTORY,
            TerminalError::ShellNotFound(_) | TerminalError::Spawn(_) => {
                codes::TERMINAL_SPAWN_FAILED
            }
            TerminalError::LimitReached | TerminalError::TooManyAttachments => {
                codes::TERMINAL_LIMIT_REACHED
            }
            TerminalError::InputTooLarge => codes::TERMINAL_INPUT_TOO_LARGE,
            TerminalError::Io(_) => codes::INTERNAL,
        };
        Self::new(code, error.to_string())
    }
}

impl From<std::io::Error> for AgentError {
    fn from(error: std::io::Error) -> Self {
        Self::new(
            codes::INTERNAL,
            format!("cannot write the run's files: {error}"),
        )
    }
}

impl From<LaunchError> for AgentError {
    fn from(error: LaunchError) -> Self {
        let code = match error {
            LaunchError::PromptTooLong
            | LaunchError::PromptHasNul
            | LaunchError::PromptHasControl => codes::AGENT_PROMPT_INVALID,
            LaunchError::NotResumable => codes::AGENT_RESUME_UNAVAILABLE,
        };
        Self::new(code, error.to_string())
    }
}

/// What the agents service is built from.
#[derive(Debug, Clone)]
pub struct AgentsConfig {
    pub store: Store,
    pub terminals: TerminalManager,
    pub workspaces: Arc<Workspaces>,
    /// The profile directory: kinds, credential homes and run files live under it.
    pub profile_dir: PathBuf,
    /// The `massorca` executable that runs the service; agents and their hooks call it.
    pub massorca_bin: PathBuf,
}

/// The agents service. Cheap to clone; every clone is the same service.
#[derive(Clone)]
pub struct Agents {
    inner: Arc<Inner>,
}

impl fmt::Debug for Agents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Agents")
            .field("profile_dir", &self.inner.config.profile_dir)
            .finish_non_exhaustive()
    }
}

struct Inner {
    config: AgentsConfig,
    /// The status store's lock and its live runs: session id to the run's hook token and rules (0053). The token is
    /// held in memory only.
    status: Mutex<LiveRuns>,
    listener: OnceLock<Listener>,
    /// Serialises starts, so the one-writer check and the request id check see every earlier start.
    start_gate: Mutex<()>,
}

/// A run whose exit the service has not seen yet.
struct LiveRun {
    token: String,
    rules: HookRules,
    /// How a prompt is typed into the run (0054).
    prompt: PromptRules,
    /// A first prompt given at launch, held until the run's first `idle`.
    pending: Option<PendingPrompt>,
    /// A prompt was typed since the run's last accepted hook report, so the next one waits for a report (decision
    /// 0058). Only a kind with a status mechanism sets it: nothing reports back otherwise.
    typed_since_report: bool,
}

/// What a prompt needs from its run's kind, copied when the run starts.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PromptRules {
    /// The kind reports activity, so a prompt waits for `idle` or `done`; without a mechanism every prompt is ready.
    has_status: bool,
    paste: PasteMode,
    submit: String,
}

impl PromptRules {
    fn of(decl: &KindDecl) -> Self {
        Self {
            has_status: decl.status.mechanism != StatusMechanism::None,
            paste: decl.prompt.paste,
            submit: decl.prompt.submit.clone(),
        }
    }
}

/// A first prompt waiting for its run's first `idle` (0054). Its text is never logged, so it has no `Debug`; it is
/// dropped when the run exits, or when its wait ends.
struct PendingPrompt {
    text: String,
    due: Instant,
}

impl PendingPrompt {
    fn new(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            due: Instant::now() + FIRST_PROMPT_WAIT,
        }
    }
}

type LiveRuns = HashMap<String, LiveRun>;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Agents {
    /// The service, with its exit sweep running every [`SWEEP_INTERVAL`] for as long as the service exists.
    pub fn new(config: AgentsConfig) -> Self {
        let agents = Self {
            inner: Arc::new(Inner {
                config,
                status: Mutex::new(HashMap::new()),
                listener: OnceLock::new(),
                start_gate: Mutex::new(()),
            }),
        };
        let weak = Arc::downgrade(&agents.inner);
        let _ = thread::Builder::new()
            .name("agent-status-sweep".to_owned())
            .spawn(move || sweep_loop(&weak));
        agents
    }

    /// Sets the receiver of every status change. Only the first call takes effect.
    pub fn set_listener(&self, listener: Listener) {
        let _ = self.inner.listener.set(listener);
    }

    fn notify(&self, change: Change) {
        if let Some(listener) = self.inner.listener.get() {
            listener(change);
        }
    }

    fn store(&self) -> &Store {
        &self.inner.config.store
    }

    fn profile_dir(&self) -> &Path {
        &self.inner.config.profile_dir
    }

    /// Every declared kind, read again from the profile, with programs resolved on the service's `PATH`.
    pub fn kinds(&self) -> AgentKinds {
        let profile_dir = self.profile_dir();
        AgentKinds {
            kinds: Catalog::load(profile_dir).views(profile_dir, &SearchEnv::current()),
        }
    }

    /// Starts a session: its first run in a new terminal in the workspace root (0051).
    pub fn start(&self, params: AgentStartParams) -> Result<AgentSession, AgentError> {
        let _gate = lock(&self.inner.start_gate);
        if let Some(request_id) = &params.request_id
            && let Some(row) = self.store().find_agent_session_by_request_id(request_id)?
        {
            return self.view(row);
        }
        let workspace = self
            .inner
            .config
            .workspaces
            .show_workspace(&params.workspace)?;
        if workspace.missing {
            return Err(AgentError::new(
                codes::PATH_NOT_FOUND,
                format!("the workspace directory is missing: {}", workspace.path),
            ));
        }
        let (decl, program) = self.startable_kind(&params.kind)?;
        if let Some(prompt) = &params.prompt {
            launch::check_prompt(prompt)?;
            // A kind that takes no prompt at launch gets it at its first idle, which only a status mechanism reports.
            if decl.prompt.mode == PromptMode::None && !decl.reports_idle() {
                return Err(AgentError::new(
                    codes::AGENT_PROMPT_INVALID,
                    format!(
                        "kind {} takes no prompt at launch and reports no idle state to send it on",
                        decl.id
                    ),
                ));
            }
        }
        if !params.read_only
            && let Some(holder) = self.live_writer(&workspace.id)?
        {
            let mut error = AgentError::new(
                codes::AGENT_WORKSPACE_BUSY,
                format!("session {holder} already writes in this workspace"),
            );
            error.data = Some(json!({ "session": holder }));
            return Err(error);
        }

        let session_id = new_id();
        let token = new_token();
        let new_ref = uuid::Uuid::new_v4().to_string();
        let home = paths::valid_home(self.profile_dir(), &decl.id).filter(|home| home.is_dir());
        let plan = launch::plan(&LaunchInput {
            decl: &decl,
            program: &program,
            session_id: &session_id,
            token: &token,
            run: Run::First {
                prompt: params.prompt.as_deref(),
                new_ref: &new_ref,
            },
            profile_dir: self.profile_dir(),
            massorca_bin: &self.inner.config.massorca_bin,
            credential_home: home.as_deref(),
            caller_args: &params.args,
            caller_env: &params.env,
        })?;

        let run_dir = paths::run_dir(self.profile_dir(), &session_id);
        // The run is registered before its program starts, and the status lock is held until its row exists, so the
        // run's first hook report waits for both instead of being refused.
        let mut live = lock(&self.inner.status);
        live.insert(
            session_id.clone(),
            LiveRun {
                token,
                rules: HookRules::of(&decl),
                prompt: PromptRules::of(&decl),
                pending: plan.pending_prompt.as_deref().map(PendingPrompt::new),
                typed_since_report: false,
            },
        );
        let agent_ref = plan.agent_ref.clone();
        let size = PtySize {
            cols: params.cols.unwrap_or(DEFAULT_COLS),
            rows: params.rows.unwrap_or(DEFAULT_ROWS),
        };
        let title = params.title.clone().unwrap_or_else(|| decl.name.clone());
        let terminal_id = self
            .spawn_run(plan, &workspace.id, &workspace.path, title, size)
            .inspect_err(|_| {
                live.remove(&session_id);
                remove_dir(&run_dir);
            })?;

        let row = AgentSessionRow {
            id: session_id.clone(),
            kind: decl.id.clone(),
            workspace_id: workspace.id,
            parent_id: None,
            title: params.title,
            read_only: params.read_only,
            agent_ref,
            terminal_id: Some(terminal_id.clone()),
            request_id: params.request_id,
            created_at: now_ms(),
            exit: None,
            activity: None,
            result: None,
            brought_back_at: None,
        };
        if let Err(error) = self.store().insert_agent_session(&row) {
            live.remove(&session_id);
            // Closed once the status lock is released: a close can wait for the terminal's kill grace.
            drop(live);
            let _ = self.inner.config.terminals.close(&terminal_id);
            remove_dir(&run_dir);
            return Err(error.into());
        }
        let session = self.view_with(&mut live, row)?;
        self.notify(Change::Updated(Box::new(session.clone())));
        Ok(session)
    }

    /// Starts a new run of an exited session, in the same session (0054): the kind's resume arguments, with the agent's
    /// ref in place of `{ref}`, in a new terminal in the workspace root. The session keeps its id and its ref; its exit
    /// and activity are cleared, so the activity starts absent until the agent reports. Refused with `agent.busy` while
    /// its run is live; `agent.resume_unavailable` when the kind is no longer declared, has no resume form, or the
    /// agent's ref is not known; `agent.not_installed` when no program of the kind resolves; `agent.workspace_busy` when
    /// another session writes in the workspace.
    pub fn resume(&self, selector: &str) -> Result<AgentSession, AgentError> {
        let _gate = lock(&self.inner.start_gate);
        let row = self.resolve(selector)?;
        if self.view(row.clone())?.execution.state != TerminalState::Exited {
            return Err(AgentError::new(
                codes::AGENT_BUSY,
                format!("session {} is still running; stop it first", row.id),
            ));
        }
        let row = self.reload(&row.id)?;
        // A session whose last run has no recorded end (no terminal and no exit) has nothing to resume from (decision 0058).
        if row.exit.is_none() {
            return Err(resume_unavailable(
                &row.id,
                "its last run has no recorded end",
            ));
        }
        let agent_ref = row
            .agent_ref
            .clone()
            .ok_or_else(|| resume_unavailable(&row.id, "its agent ref is not known"))?;
        let (decl, program) = self.resumable_kind(&row)?;
        let workspace = self
            .inner
            .config
            .workspaces
            .show_workspace(&row.workspace_id)?;
        if workspace.missing {
            return Err(AgentError::new(
                codes::PATH_NOT_FOUND,
                format!("the workspace directory is missing: {}", workspace.path),
            ));
        }
        if !row.read_only
            && let Some(holder) = self.live_writer(&workspace.id)?
        {
            let mut error = AgentError::new(
                codes::AGENT_WORKSPACE_BUSY,
                format!("session {holder} already writes in this workspace"),
            );
            error.data = Some(json!({ "session": holder }));
            return Err(error);
        }

        let token = new_token();
        let home = paths::valid_home(self.profile_dir(), &decl.id).filter(|home| home.is_dir());
        // A resumed run takes the kind's resume arguments only: the caller's arguments and environment of the first start
        // are not kept with the session (docs/architecture/agents.md).
        let no_env = BTreeMap::new();
        let plan = launch::plan(&LaunchInput {
            decl: &decl,
            program: &program,
            session_id: &row.id,
            token: &token,
            run: Run::Resume {
                agent_ref: &agent_ref,
            },
            profile_dir: self.profile_dir(),
            massorca_bin: &self.inner.config.massorca_bin,
            credential_home: home.as_deref(),
            caller_args: &[],
            caller_env: &no_env,
        })?;
        let run_dir = paths::run_dir(self.profile_dir(), &row.id);
        // As in `start`: the run is registered under the status lock before its program starts, and the lock is held until
        // the row names the new terminal, so the run's first report waits instead of being refused.
        let mut live = lock(&self.inner.status);
        live.insert(
            row.id.clone(),
            LiveRun {
                token,
                rules: HookRules::of(&decl),
                prompt: PromptRules::of(&decl),
                pending: None,
                typed_since_report: false,
            },
        );
        let title = row.title.clone().unwrap_or_else(|| decl.name.clone());
        let size = PtySize {
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
        };
        let terminal_id = self
            .spawn_run(plan, &workspace.id, &workspace.path, title, size)
            .inspect_err(|_| {
                live.remove(&row.id);
                remove_dir(&run_dir);
            })?;
        let restarted = self.store().restart_agent_session(&row.id, &terminal_id);
        if !matches!(restarted, Ok(true)) {
            live.remove(&row.id);
            // Closed once the status lock is released: a close can wait for the terminal's kill grace.
            drop(live);
            let _ = self.inner.config.terminals.close(&terminal_id);
            remove_dir(&run_dir);
            restarted?;
            return Err(AgentError::new(
                codes::AGENT_NOT_FOUND,
                format!("no agent session {}", row.id),
            ));
        }
        let fresh = self.reload(&row.id)?;
        let session = self.view_with(&mut live, fresh)?;
        self.notify(Change::Updated(Box::new(session.clone())));
        Ok(session)
    }

    /// Writes a run's files, then creates its terminal in the workspace root with the launch plan's program, arguments
    /// and environment (0051). Returns the terminal's id.
    fn spawn_run(
        &self,
        plan: LaunchPlan,
        workspace_id: &str,
        cwd: &str,
        title: String,
        size: PtySize,
    ) -> Result<String, AgentError> {
        if !plan.files.is_empty() {
            crate::credentials::create_agents_dir(self.profile_dir())?;
        }
        write_files(&plan.files)?;
        let mut env = plan.env;
        // The terminal can only add variables, so a stripped one is set to empty instead (docs/architecture/agents.md).
        for name in plan.remove_env {
            env.entry(name).or_default();
        }
        let spec = CreateSpec {
            workspace_id: workspace_id.to_owned(),
            cwd: PathBuf::from(cwd),
            shell: Some(ShellChoice {
                program: plan.program.to_string_lossy().into_owned(),
                args: plan.args,
            }),
            env,
            size,
            title: Some(title),
            request_id: None,
        };
        Ok(self.inner.config.terminals.create(spec)?.id)
    }

    /// The declaration and program a resume starts with: declared, valid, with a resume form, and installed (0054).
    fn resumable_kind(&self, row: &AgentSessionRow) -> Result<(KindDecl, PathBuf), AgentError> {
        let catalog = Catalog::load(self.profile_dir());
        let entry = catalog.find(&row.kind).ok_or_else(|| {
            resume_unavailable(&row.id, &format!("kind {} is no longer declared", row.kind))
        })?;
        let decl = entry
            .valid()
            .ok_or_else(|| resume_unavailable(&row.id, &format!("kind {} is invalid", row.kind)))?;
        if !decl.resumable() {
            return Err(resume_unavailable(
                &row.id,
                &format!("kind {} declares no resume form", row.kind),
            ));
        }
        let program = resolve_first(&decl.program, &SearchEnv::current()).ok_or_else(|| {
            AgentError::new(
                codes::AGENT_NOT_INSTALLED,
                format!(
                    "agent kind {} is not installed: none of its programs was found",
                    row.kind
                ),
            )
        })?;
        Ok((decl.clone(), program))
    }

    /// Whether `token` is the run token of a live run of `session`, compared in constant time.
    pub fn run_token_matches(&self, session: &str, token: &str) -> bool {
        lock(&self.inner.status)
            .get(session)
            .is_some_and(|run| tokens_match(&run.token, token))
    }

    /// Applies one hook report (0053). An unknown session, a run that has ended, or a wrong token is refused with
    /// `agent.hook_refused` and changes nothing; an event the kind does not map, or maps to `ignore`, changes nothing.
    pub fn hook(&self, params: &AgentHookParams) -> Result<(), AgentError> {
        if params
            .payload
            .as_ref()
            .is_some_and(|payload| payload.len() > MAX_HOOK_PAYLOAD_BYTES)
        {
            return Err(AgentError::new(
                codes::REQUEST_INVALID_PARAMS,
                format!("a hook payload holds at most {MAX_HOOK_PAYLOAD_BYTES} bytes"),
            ));
        }
        let mut live = lock(&self.inner.status);
        let rules = match live.get(&params.session) {
            Some(run) if tokens_match(&run.token, &params.token) => run.rules.clone(),
            _ => {
                return Err(AgentError::new(
                    codes::AGENT_HOOK_REFUSED,
                    "the report does not match a live run of this session",
                ));
            }
        };
        let Some(report) =
            status::report(&rules, &params.event, params.payload.as_deref(), now_ms())
        else {
            return Ok(());
        };
        let first_idle = report.activity.state == "idle";
        let mut row = self.reload(&params.session)?;
        let result = if report.activity.state == "done" {
            report.result
        } else {
            row.result.take()
        };
        let learned = report.learned_ref.filter(|_| row.agent_ref.is_none());
        self.store().set_agent_session_status(
            &row.id,
            Some(&report.activity),
            result.as_deref(),
            learned.as_deref(),
        )?;
        row.activity = Some(report.activity);
        row.result = result;
        if row.agent_ref.is_none() {
            row.agent_ref = learned;
        }
        // The report answers whatever was typed before it, so the run's typed-since-report flag clears (decision 0058).
        if let Some(run) = live.get_mut(&params.session) {
            run.typed_since_report = false;
        }
        let session = self.view_with(&mut live, row)?;
        // The run's first `idle` sends the first prompt it was given at launch (0054). The text leaves the run under the
        // lock and is written after the lock is released; taking it counts as typing it (decision 0058).
        let first = if first_idle {
            live.get_mut(&params.session).and_then(|run| {
                let pending = run.pending.take()?;
                run.typed_since_report = true;
                Some((run.prompt.clone(), pending.text))
            })
        } else {
            None
        };
        let terminal = session.terminal_id.clone();
        self.notify(Change::Updated(Box::new(session)));
        drop(live);
        if let (Some((rules, text)), Some(terminal)) = (first, terminal) {
            self.deliver_first_prompt(&params.session, &terminal, &rules, &text);
        }
        Ok(())
    }

    /// Types a prompt into a live run's terminal (0054): the text, then the kind's submit sequence. The text is refused
    /// over the size limit or with a NUL. Unless `force` is set, the run must be ready (see [`prompt::ready`]). The
    /// receipt counts the bytes written; it never says the agent read them.
    pub fn prompt(
        &self,
        selector: &str,
        text: &str,
        force: bool,
    ) -> Result<PromptReceipt, AgentError> {
        launch::check_prompt(text)?;
        let row = self.resolve(selector)?;
        // The readiness check and the copy of the input rules happen under the status lock. The write does not: it can
        // wait for the agent to read, and the agent's own hook reports need the lock to return.
        let (session_id, terminal, bytes) = {
            let mut live = lock(&self.inner.status);
            let row = self.reload(&row.id)?;
            let session = self.view_with(&mut live, row)?;
            let Some(run) = live.get_mut(&session.id) else {
                return Err(not_live(&session.id));
            };
            let state = session.activity.as_ref().map(|activity| activity.state);
            // A run that has taken a prompt since its last report is not ready either: its agent may not have read that
            // prompt yet, so a second one would land on the same idle (decision 0058).
            if !force && (!prompt::ready(run.prompt.has_status, state) || run.typed_since_report) {
                return Err(AgentError::new(
                    codes::AGENT_BUSY,
                    format!(
                        "session {} is not ready for a prompt: wait for its agent to report, or use force",
                        session.id
                    ),
                ));
            }
            let bytes = prompt::input_bytes(text, run.prompt.paste, &run.prompt.submit);
            let Some(terminal) = session.terminal_id.clone() else {
                return Err(not_live(&session.id));
            };
            // Accepted under the lock, so a second prompt on the same idle sees the flag. A kind with no status mechanism
            // never hears back, so it never sets the flag: it stays always ready (0054).
            run.typed_since_report = run.prompt.has_status;
            (session.id.clone(), terminal, bytes)
        };
        self.inner
            .config
            .terminals
            .input(&terminal, &bytes)
            .map_err(|error| match error {
                TerminalError::Exited | TerminalError::NotFound => not_live(&session_id),
                other => AgentError::from(other),
            })?;
        Ok(PromptReceipt {
            receipt: ReceiptKind::HandedToTerminal,
            bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        })
    }

    /// Types a first prompt on a thread of its own. The write can wait for the agent to read, and the agent waits for the
    /// hook report that carried this `idle` to return, so the report must not wait for the write (0054).
    fn deliver_first_prompt(&self, session: &str, terminal: &str, rules: &PromptRules, text: &str) {
        let bytes = prompt::input_bytes(text, rules.paste, &rules.submit);
        let terminals = self.inner.config.terminals.clone();
        let terminal = terminal.to_owned();
        let owner = session.to_owned();
        let logged = owner.clone();
        let spawned = thread::Builder::new()
            .name("agent-first-prompt".to_owned())
            .spawn(move || {
                if let Err(error) = terminals.input(&terminal, &bytes) {
                    let error = AgentError::from(error);
                    tracing::warn!(
                        session = %owner,
                        code = error.code,
                        "the first prompt could not be typed into the run's terminal"
                    );
                }
            });
        if let Err(error) = spawned {
            tracing::warn!(session = %logged, %error, "the first prompt has no thread to be typed on");
        }
    }

    /// Checks the terminal of every live run once and applies the exits it finds, in order with hook reports. Each
    /// session is checked on its own: one that fails is logged, by its id and error code only, and the sweep goes on.
    pub fn sweep(&self) {
        let mut live = lock(&self.inner.status);
        for id in expire_first_prompts(&mut live, Instant::now()) {
            tracing::warn!(
                session = %id,
                "the first prompt was dropped: the agent reported no idle state in time"
            );
        }
        let ids: Vec<String> = live.keys().cloned().collect();
        for id in ids {
            let checked = match self.store().get_agent_session(&id) {
                Ok(Some(row)) => self.view_with(&mut live, row).map(drop),
                Ok(None) => {
                    live.remove(&id);
                    Ok(())
                }
                Err(error) => Err(AgentError::from(error)),
            };
            if let Err(error) = checked {
                tracing::warn!(session = %id, code = error.code, "the exit sweep could not check a session");
            }
        }
    }

    /// Sessions newest first, optionally only those of one workspace (a selector).
    pub fn list(&self, workspace: Option<&str>) -> Result<Vec<AgentSession>, AgentError> {
        let workspace_id = match workspace {
            Some(selector) => Some(self.inner.config.workspaces.show_workspace(selector)?.id),
            None => None,
        };
        self.store()
            .list_agent_sessions(workspace_id.as_deref())?
            .into_iter()
            .map(|row| self.view(row))
            .collect()
    }

    /// The id of a workspace, by id, name or path.
    pub fn workspace_id(&self, selector: &str) -> Result<String, AgentError> {
        Ok(self.inner.config.workspaces.show_workspace(selector)?.id)
    }

    /// One session, by id or by a unique id prefix of at least [`MIN_SELECTOR_PREFIX`] characters.
    pub fn show(&self, selector: &str) -> Result<AgentSession, AgentError> {
        let row = self.resolve(selector)?;
        self.view(row)
    }

    /// Ends the current run (graceful, or at once with `force`). The record stays, exited with reason `killed`.
    /// A session whose run already ended is returned as it is.
    pub fn stop(&self, selector: &str, force: bool) -> Result<AgentSession, AgentError> {
        let row = self.resolve(selector)?;
        if row.exit.is_none()
            && let Some(terminal) = &row.terminal_id
        {
            match self.inner.config.terminals.kill(terminal, force) {
                Ok(info) => {
                    if let Some(exit) = info.exit {
                        let mut live = lock(&self.inner.status);
                        let mut row = self.reload(&row.id)?;
                        if row.exit.is_none() {
                            self.record_exit(&mut live, &mut row, terminal, run_exit_of(&exit))?;
                        }
                    }
                }
                Err(TerminalError::NotFound) => {}
                Err(error) => return Err(error.into()),
            }
        }
        let row = self.reload(&row.id)?;
        self.view(row)
    }

    /// Deletes the record of a session whose run has ended, with its terminal and generated files.
    pub fn forget(&self, selector: &str) -> Result<AgentSession, AgentError> {
        let id = self.resolve(selector)?.id;
        let mut live = lock(&self.inner.status);
        let row = self.reload(&id)?;
        let session = self.view_with(&mut live, row)?;
        if session.execution.state != TerminalState::Exited {
            return Err(AgentError::new(
                codes::AGENT_BUSY,
                format!("session {} is still running; stop it first", session.id),
            ));
        }
        self.store().delete_agent_session(&session.id)?;
        live.remove(&session.id);
        self.notify(Change::Forgotten(session.id.clone()));
        // Closed once the status lock is released: a close can wait for the terminal's kill grace.
        drop(live);
        if let Some(terminal) = &session.terminal_id {
            let _ = self.inner.config.terminals.close(terminal);
        }
        remove_dir(&paths::run_dir(self.profile_dir(), &session.id));
        Ok(session)
    }

    /// Marks every session whose run was live when the service last stopped as exited with reason `service_stopped`
    /// (0051). Called once at service start, before any run exists. Returns how many sessions changed.
    /// A run that was `working` or `waiting` then is `done` with outcome `interrupted` (0052).
    pub fn mark_stale_on_startup(&self) -> Result<u64, AgentError> {
        let at = now_ms();
        for row in self.store().list_agent_sessions(None)? {
            if row.exit.is_none()
                && let Some(activity) = status::on_exit(row.activity.as_ref(), at)
            {
                self.store().set_agent_session_status(
                    &row.id,
                    Some(&activity),
                    row.result.as_deref(),
                    None,
                )?;
            }
        }
        Ok(self.store().mark_agent_runs_service_stopped(at)?)
    }

    fn startable_kind(&self, id: &str) -> Result<(crate::KindDecl, PathBuf), AgentError> {
        let catalog = Catalog::load(self.profile_dir());
        let entry = catalog.find(id).ok_or_else(|| {
            AgentError::new(codes::AGENT_KIND_NOT_FOUND, format!("no agent kind {id}"))
        })?;
        let decl = entry.valid().ok_or_else(|| {
            AgentError::new(
                codes::AGENT_NOT_INSTALLED,
                format!("agent kind {id} is invalid: {}", entry.problems.join("; ")),
            )
        })?;
        let program = resolve_first(&decl.program, &SearchEnv::current()).ok_or_else(|| {
            AgentError::new(
                codes::AGENT_NOT_INSTALLED,
                format!("agent kind {id} is not installed: none of its programs was found"),
            )
        })?;
        Ok((decl.clone(), program))
    }

    /// The live writing session of a workspace, if any.
    fn live_writer(&self, workspace_id: &str) -> Result<Option<String>, AgentError> {
        for row in self.store().list_agent_sessions(Some(workspace_id))? {
            if row.read_only {
                continue;
            }
            let session = self.view(row)?;
            if session.execution.state != TerminalState::Exited {
                return Ok(Some(session.id));
            }
        }
        Ok(None)
    }

    fn resolve(&self, selector: &str) -> Result<AgentSessionRow, AgentError> {
        if let Some(row) = self.store().get_agent_session(selector)? {
            return Ok(row);
        }
        if selector.len() >= MIN_SELECTOR_PREFIX {
            let mut rows = self.store().find_agent_sessions_by_prefix(selector, 2)?;
            match rows.len() {
                1 => return Ok(rows.remove(0)),
                0 => {}
                _ => {
                    return Err(AgentError::new(
                        codes::SELECTOR_AMBIGUOUS,
                        format!("{selector} is the start of more than one session id"),
                    ));
                }
            }
        }
        Err(AgentError::new(
            codes::AGENT_NOT_FOUND,
            format!("no agent session {selector}"),
        ))
    }

    fn reload(&self, id: &str) -> Result<AgentSessionRow, AgentError> {
        self.store().get_agent_session(id)?.ok_or_else(|| {
            AgentError::new(codes::AGENT_NOT_FOUND, format!("no agent session {id}"))
        })
    }

    /// The record of a row read before the status lock: the row is read again under the lock, so a change applied
    /// in between is neither lost nor overwritten.
    fn view(&self, row: AgentSessionRow) -> Result<AgentSession, AgentError> {
        let mut live = lock(&self.inner.status);
        let row = self.store().get_agent_session(&row.id)?.unwrap_or(row);
        self.view_with(&mut live, row)
    }

    /// The record of a row, with execution read from its terminal now. An end the terminal reports, or a terminal
    /// that is gone without one, is applied as a status change, so it survives the terminal.
    fn view_with(
        &self,
        live: &mut LiveRuns,
        mut row: AgentSessionRow,
    ) -> Result<AgentSession, AgentError> {
        let execution = match (&row.exit, &row.terminal_id) {
            (Some(exit), _) => {
                live.remove(&row.id);
                exited(exit)
            }
            (None, None) => Execution {
                state: TerminalState::Exited,
                exit: None,
            },
            (None, Some(terminal)) => {
                let terminal = terminal.clone();
                match self.inner.config.terminals.get(&terminal) {
                    Ok(info) => match info.state {
                        CoreState::Live => running(TerminalState::Live),
                        CoreState::Unverifiable => running(TerminalState::Unverifiable),
                        CoreState::Exited => {
                            let exit = info.exit.as_ref().map_or_else(gone, run_exit_of);
                            self.record_exit(live, &mut row, &terminal, exit)?
                        }
                    },
                    Err(TerminalError::NotFound) => {
                        self.record_exit(live, &mut row, &terminal, gone())?
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        };
        Ok(record(row, execution))
    }

    /// Applies a run's end: the exit, and the interrupted rule (0052), written to the row and announced once. The end is
    /// recorded only while the row still names `terminal`, the terminal the run ran in (decision 0058): a resumed session
    /// has a new terminal, and an earlier run's end must neither land on it nor end its live run.
    fn record_exit(
        &self,
        live: &mut LiveRuns,
        row: &mut AgentSessionRow,
        terminal: &str,
        exit: RunExit,
    ) -> Result<Execution, AgentError> {
        let changed = self
            .store()
            .set_agent_session_exit(&row.id, terminal, &exit)?;
        if changed {
            live.remove(&row.id);
            // The exit rule reads the activity and result as they are now, under the status lock.
            if let Some(fresh) = self.store().get_agent_session(&row.id)? {
                *row = fresh;
            }
            if let Some(activity) = status::on_exit(row.activity.as_ref(), exit.at) {
                self.store().set_agent_session_status(
                    &row.id,
                    Some(&activity),
                    row.result.as_deref(),
                    None,
                )?;
                row.activity = Some(activity);
            }
        }
        let execution = exited(&exit);
        row.exit = Some(exit);
        if changed {
            self.notify(Change::Updated(Box::new(record(
                row.clone(),
                execution.clone(),
            ))));
        }
        Ok(execution)
    }
}

/// The wire record of a row with its execution.
fn record(row: AgentSessionRow, execution: Execution) -> AgentSession {
    AgentSession {
        id: row.id,
        kind: row.kind,
        workspace_id: row.workspace_id,
        title: row.title,
        read_only: row.read_only,
        parent_id: row.parent_id,
        brought_back_at_ms: row.brought_back_at,
        agent_ref: row.agent_ref,
        terminal_id: row.terminal_id,
        created_at_ms: row.created_at,
        execution,
        activity: row.activity.as_ref().and_then(activity_of),
        result: row.result,
    }
}

/// The refusal for a resume the session cannot make (0054).
fn resume_unavailable(session: &str, reason: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_RESUME_UNAVAILABLE,
        format!("session {session} cannot resume: {reason}"),
    )
}

/// The refusal for a prompt to a run that has ended. The session's id only, never the prompt.
fn not_live(session: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_NOT_LIVE,
        format!("the run of session {session} has exited"),
    )
}

/// Compares two tokens without stopping at the first difference.
fn tokens_match(expected: &str, given: &str) -> bool {
    expected.len() == given.len()
        && expected
            .bytes()
            .zip(given.bytes())
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0
}

/// Drops every first prompt whose wait has ended by `now`; returns the sessions they belonged to (0054).
fn expire_first_prompts(live: &mut LiveRuns, now: Instant) -> Vec<String> {
    let mut expired = Vec::new();
    for (id, run) in live.iter_mut() {
        if run
            .pending
            .as_ref()
            .is_some_and(|pending| now >= pending.due)
        {
            run.pending = None;
            expired.push(id.clone());
        }
    }
    expired
}

/// Sweeps every [`SWEEP_INTERVAL`] until the service is gone.
fn sweep_loop(service: &Weak<Inner>) {
    loop {
        thread::sleep(SWEEP_INTERVAL);
        let Some(inner) = service.upgrade() else {
            return;
        };
        let agents = Agents { inner };
        if !lock(&agents.inner.status).is_empty() {
            agents.sweep();
        }
    }
}

/// A random hook token of 244 bits, as 64 hex digits.
fn new_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn write_files(files: &[(PathBuf, String)]) -> std::io::Result<()> {
    for (path, contents) in files {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)?;
    }
    Ok(())
}

fn remove_dir(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn running(state: TerminalState) -> Execution {
    Execution { state, exit: None }
}

/// The end of a run whose terminal is gone without an exit the service saw (closed, or dropped from the list of
/// exited terminals): `exited`, with no code, at the time it was noticed.
fn gone() -> RunExit {
    RunExit {
        code: None,
        signal: None,
        reason: RunExitReason::Exited,
        at: now_ms(),
    }
}

fn run_exit_of(exit: &ExitRecord) -> RunExit {
    RunExit {
        code: exit.code,
        signal: exit.signal.clone(),
        reason: match exit.reason {
            CoreExitReason::Exited => RunExitReason::Exited,
            CoreExitReason::Killed => RunExitReason::Killed,
            CoreExitReason::ServiceStopped => RunExitReason::ServiceStopped,
        },
        at: exit.at_ms,
    }
}

fn exited(exit: &RunExit) -> Execution {
    Execution {
        state: TerminalState::Exited,
        exit: Some(TerminalExit {
            code: exit.code,
            signal: exit.signal.clone(),
            reason: match exit.reason {
                RunExitReason::Exited => ExitReason::Exited,
                RunExitReason::Killed => ExitReason::Killed,
                RunExitReason::ServiceStopped => ExitReason::ServiceStopped,
            },
            at_ms: exit.at,
        }),
    }
}

/// The stored activity in wire form; values a newer build wrote read as `unknown`.
fn activity_of(stored: &StoredActivity) -> Option<Activity> {
    let mut value = json!({
        "state": stored.state,
        "source": stored.source,
        "at_ms": stored.at,
    });
    if let Some(outcome) = &stored.outcome {
        value["outcome"] = json!(outcome);
    }
    serde_json::from_value(value).ok()
}

#[cfg(test)]
mod tests {
    use massorca_core_git::Git;
    use massorca_core_store::{
        ProjectKind, ProjectRow, WorkspaceKind, WorkspaceRow, WorkspaceState,
    };
    use massorca_core_terminal::ManagerConfig;
    use massorca_core_terminal::shell::EnvSnapshot;
    use massorca_protocol::methods::agents::MAX_PROMPT_BYTES;

    use super::*;

    /// A service over an in-memory store with one workspace; returns it, the workspace id and the profile directory.
    fn service() -> (Agents, String, tempfile::TempDir) {
        let profile = tempfile::tempdir().unwrap();
        let store = Store::open_in_memory().unwrap();
        let project = ProjectRow {
            id: new_id(),
            kind: ProjectKind::Folder,
            path: "/work/app".to_owned(),
            path_key: "/work/app".to_owned(),
            name: "app".to_owned(),
            workspaces_root: "/work/app-workspaces".to_owned(),
            added_at: now_ms(),
        };
        store.insert_project(&project).unwrap();
        let workspace = WorkspaceRow {
            id: new_id(),
            project_id: project.id.clone(),
            kind: WorkspaceKind::Folder,
            name: "main".to_owned(),
            path: "/work/app".to_owned(),
            path_key: "/work/app".to_owned(),
            branch: None,
            base_ref: None,
            created_branch: false,
            state: WorkspaceState::Ready,
            request_id: None,
            created_at: now_ms(),
        };
        store.insert_workspace(&workspace).unwrap();
        let agents = Agents::new(AgentsConfig {
            store: store.clone(),
            terminals: TerminalManager::new(ManagerConfig::new(EnvSnapshot::current(), "test")),
            workspaces: Arc::new(Workspaces::new(
                store,
                Git::with_binary("git"),
                profile.path().join("workspaces"),
            )),
            profile_dir: profile.path().to_path_buf(),
            massorca_bin: PathBuf::from("massorca"),
        });
        (agents, workspace.id, profile)
    }

    fn activity(state: &str, outcome: Option<&str>) -> StoredActivity {
        StoredActivity {
            state: state.to_owned(),
            outcome: outcome.map(str::to_owned),
            source: "hook".to_owned(),
            at: 5,
        }
    }

    /// A row read before a `done` report landed must not turn that report into an interrupted turn when the exit is
    /// applied: the exit rule reads the row as it is under the status lock.
    #[test]
    fn an_exit_applied_through_a_stale_row_keeps_the_done_report_and_its_result() {
        let (agents, workspace_id, _profile) = service();
        let stale = AgentSessionRow {
            id: new_id(),
            kind: "fake".to_owned(),
            workspace_id,
            parent_id: None,
            title: None,
            read_only: false,
            agent_ref: None,
            // A terminal the manager does not know: viewing the row applies the run's end.
            terminal_id: Some(new_id()),
            request_id: None,
            created_at: now_ms(),
            exit: None,
            activity: Some(activity("working", None)),
            result: None,
            brought_back_at: None,
        };
        agents.store().insert_agent_session(&stale).unwrap();
        agents
            .store()
            .set_agent_session_status(
                &stale.id,
                Some(&activity("done", Some("success"))),
                Some("the answer"),
                None,
            )
            .unwrap();

        let session = agents.view(stale.clone()).unwrap();

        assert_eq!(session.execution.state, TerminalState::Exited);
        let shown = session.activity.expect("activity");
        assert_eq!(
            shown.state,
            massorca_protocol::methods::agents::ActivityState::Done
        );
        assert_eq!(
            shown.outcome,
            Some(massorca_protocol::methods::agents::Outcome::Success)
        );
        assert_eq!(session.result.as_deref(), Some("the answer"));
        let stored = agents
            .store()
            .get_agent_session(&stale.id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.activity, Some(activity("done", Some("success"))));
        assert_eq!(stored.result.as_deref(), Some("the answer"));
    }

    /// A prompt with a NUL or over the size limit is refused before its session is looked up (0054).
    #[test]
    fn a_prompt_is_checked_before_its_session_is_looked_up() {
        let (agents, _workspace_id, _profile) = service();
        let nul = agents.prompt("no-such-session", "a\0b", false).unwrap_err();
        assert_eq!(nul.code, codes::AGENT_PROMPT_INVALID);
        let long = "a".repeat(MAX_PROMPT_BYTES + 1);
        let over = agents.prompt("no-such-session", &long, false).unwrap_err();
        assert_eq!(over.code, codes::AGENT_PROMPT_INVALID);
        let missing = agents.prompt("no-such-session", "fine", false).unwrap_err();
        assert_eq!(missing.code, codes::AGENT_NOT_FOUND);
    }

    /// A first prompt is dropped once its wait has ended; a run still inside its wait keeps it (0054).
    #[test]
    fn a_first_prompt_is_dropped_only_once_its_wait_has_ended() {
        let now = Instant::now();
        let run = |due| LiveRun {
            token: "t".to_owned(),
            rules: HookRules::default(),
            prompt: PromptRules {
                has_status: true,
                paste: PasteMode::Plain,
                submit: "\r".to_owned(),
            },
            pending: Some(PendingPrompt {
                text: "later".to_owned(),
                due,
            }),
            typed_since_report: false,
        };
        let mut live: LiveRuns = HashMap::new();
        live.insert("waiting".to_owned(), run(now + FIRST_PROMPT_WAIT));
        live.insert("late".to_owned(), run(now));

        assert_eq!(
            expire_first_prompts(&mut live, now),
            vec!["late".to_owned()]
        );
        assert!(live["waiting"].pending.is_some());
        assert!(live["late"].pending.is_none());
    }

    /// A resume of a session that does not exist is refused as not found (0054).
    #[test]
    fn a_resume_of_an_unknown_session_is_not_found() {
        let (agents, _workspace_id, _profile) = service();
        let error = agents.resume("no-such-session").unwrap_err();
        assert_eq!(error.code, codes::AGENT_NOT_FOUND);
    }

    /// An end recorded for an earlier run's terminal changes nothing once the session runs on another terminal: the
    /// resumed run stays live and unrecorded (decision 0058).
    #[test]
    fn an_end_of_an_earlier_terminal_does_not_end_the_resumed_run() {
        let (agents, workspace_id, _profile) = service();
        let row = AgentSessionRow {
            id: new_id(),
            kind: "fake".to_owned(),
            workspace_id,
            parent_id: None,
            title: None,
            read_only: false,
            agent_ref: None,
            terminal_id: Some("t-1".to_owned()),
            request_id: None,
            created_at: now_ms(),
            exit: None,
            activity: None,
            result: None,
            brought_back_at: None,
        };
        agents.store().insert_agent_session(&row).unwrap();
        let end = RunExit {
            code: Some(0),
            signal: None,
            reason: RunExitReason::Exited,
            at: 5,
        };
        // The first run ends on t-1; the resume then moves the session onto t-2, where its new run is live.
        assert!(
            agents
                .store()
                .set_agent_session_exit(&row.id, "t-1", &end)
                .unwrap()
        );
        assert!(
            agents
                .store()
                .restart_agent_session(&row.id, "t-2")
                .unwrap()
        );
        let mut live: LiveRuns = HashMap::new();
        live.insert(
            row.id.clone(),
            LiveRun {
                token: "t".to_owned(),
                rules: HookRules::default(),
                prompt: PromptRules {
                    has_status: true,
                    paste: PasteMode::Plain,
                    submit: "\r".to_owned(),
                },
                pending: None,
                typed_since_report: false,
            },
        );

        // A late record of the first run's end, as a stop that outlived the resume would apply it.
        let mut current = agents.store().get_agent_session(&row.id).unwrap().unwrap();
        agents
            .record_exit(&mut live, &mut current, "t-1", end)
            .unwrap();

        let stored = agents.store().get_agent_session(&row.id).unwrap().unwrap();
        assert_eq!(stored.exit, None);
        assert_eq!(stored.terminal_id.as_deref(), Some("t-2"));
        assert!(live.contains_key(&row.id), "the resumed run stays live");
    }

    /// A session whose last run has no recorded end (no terminal and no exit) has nothing to resume from: it is refused as
    /// unavailable, not as not found (decision 0058).
    #[test]
    fn a_session_with_no_recorded_run_end_cannot_resume() {
        let (agents, workspace_id, _profile) = service();
        let row = AgentSessionRow {
            id: new_id(),
            kind: "claude".to_owned(),
            workspace_id,
            parent_id: None,
            title: None,
            read_only: false,
            agent_ref: Some("ref-1".to_owned()),
            terminal_id: None,
            request_id: None,
            created_at: now_ms(),
            exit: None,
            activity: None,
            result: None,
            brought_back_at: None,
        };
        agents.store().insert_agent_session(&row).unwrap();

        let error = agents.resume(&row.id).unwrap_err();
        assert_eq!(error.code, codes::AGENT_RESUME_UNAVAILABLE, "{error}");
    }
}
