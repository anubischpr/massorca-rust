//! Agent sessions (decision 0051): the service object that starts runs in terminals, keeps session rows, and reads
//! each session's execution from its terminal.
//!
//! Calls block (store, process spawn), so the service runs them on its blocking pool.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

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
    Activity, AgentKinds, AgentSession, AgentStartParams, Execution,
};
use massorca_protocol::methods::terminals::{
    DEFAULT_COLS, DEFAULT_ROWS, ExitReason, TerminalExit, TerminalState,
};
use massorca_protocol::{ErrorBody, codes};
use serde_json::{Value, json};

use crate::catalog::Catalog;
use crate::launch::{self, LaunchError, LaunchInput, Run};
use crate::paths;
use crate::resolve::{SearchEnv, resolve_first};

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
            LaunchError::PromptTooLong | LaunchError::PromptHasNul => codes::AGENT_PROMPT_INVALID,
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
    /// Session id to the current run's hook token (0053). Held in memory only.
    tokens: Mutex<HashMap<String, String>>,
    /// Serialises starts, so the one-writer check and the request id check see every earlier start.
    start_gate: Mutex<()>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Agents {
    pub fn new(config: AgentsConfig) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                tokens: Mutex::new(HashMap::new()),
                start_gate: Mutex::new(()),
            }),
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
        if plan.pending_prompt.is_some() {
            return Err(AgentError::new(
                codes::AGENT_PROMPT_INVALID,
                format!("kind {} takes no prompt at launch", decl.id),
            ));
        }

        let run_dir = paths::run_dir(self.profile_dir(), &session_id);
        let started = write_files(&plan.files)
            .map_err(AgentError::from)
            .and_then(|()| {
                let mut env = plan.env;
                // The terminal can only add variables, so a stripped one is set to empty instead (docs/architecture/agents.md).
                for name in plan.remove_env {
                    env.entry(name).or_default();
                }
                let spec = CreateSpec {
                    workspace_id: workspace.id.clone(),
                    cwd: PathBuf::from(&workspace.path),
                    shell: Some(ShellChoice {
                        program: plan.program.to_string_lossy().into_owned(),
                        args: plan.args,
                    }),
                    env,
                    size: PtySize {
                        cols: params.cols.unwrap_or(DEFAULT_COLS),
                        rows: params.rows.unwrap_or(DEFAULT_ROWS),
                    },
                    title: Some(params.title.clone().unwrap_or_else(|| decl.name.clone())),
                    request_id: None,
                };
                Ok(self.inner.config.terminals.create(spec)?)
            });
        let terminal = started.inspect_err(|_| remove_dir(&run_dir))?;

        let row = AgentSessionRow {
            id: session_id.clone(),
            kind: decl.id.clone(),
            workspace_id: workspace.id,
            parent_id: None,
            title: params.title,
            read_only: params.read_only,
            agent_ref: plan.agent_ref,
            terminal_id: Some(terminal.id.clone()),
            request_id: params.request_id,
            created_at: now_ms(),
            exit: None,
            activity: None,
            result: None,
            brought_back_at: None,
        };
        if let Err(error) = self.store().insert_agent_session(&row) {
            let _ = self.inner.config.terminals.close(&terminal.id);
            remove_dir(&run_dir);
            return Err(error.into());
        }
        lock(&self.inner.tokens).insert(session_id, token);
        self.view(row)
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
                        self.store()
                            .set_agent_session_exit(&row.id, &run_exit_of(&exit))?;
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
        let row = self.resolve(selector)?;
        let session = self.view(row)?;
        if session.execution.state != TerminalState::Exited {
            return Err(AgentError::new(
                codes::AGENT_BUSY,
                format!("session {} is still running; stop it first", session.id),
            ));
        }
        self.store().delete_agent_session(&session.id)?;
        if let Some(terminal) = &session.terminal_id {
            let _ = self.inner.config.terminals.close(terminal);
        }
        remove_dir(&paths::run_dir(self.profile_dir(), &session.id));
        lock(&self.inner.tokens).remove(&session.id);
        Ok(session)
    }

    /// Marks every session whose run was live when the service last stopped as exited with reason `service_stopped`
    /// (0051). Called once at service start, before any run exists. Returns how many sessions changed.
    pub fn mark_stale_on_startup(&self) -> Result<u64, AgentError> {
        Ok(self.store().mark_agent_runs_service_stopped(now_ms())?)
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

    /// The record of a row, with execution read from its terminal now. An end the terminal reports, or a terminal
    /// that is gone without one, is written to the row, so it survives the terminal.
    fn view(&self, mut row: AgentSessionRow) -> Result<AgentSession, AgentError> {
        let execution = match (&row.exit, &row.terminal_id) {
            (Some(exit), _) => exited(exit),
            (None, None) => Execution {
                state: TerminalState::Exited,
                exit: None,
            },
            (None, Some(terminal)) => match self.inner.config.terminals.get(terminal) {
                Ok(info) => match info.state {
                    CoreState::Live => live(TerminalState::Live),
                    CoreState::Unverifiable => live(TerminalState::Unverifiable),
                    CoreState::Exited => {
                        let exit = info.exit.as_ref().map_or_else(gone, run_exit_of);
                        self.record_exit(&mut row, exit)?
                    }
                },
                Err(TerminalError::NotFound) => self.record_exit(&mut row, gone())?,
                Err(error) => return Err(error.into()),
            },
        };
        Ok(AgentSession {
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
        })
    }

    fn record_exit(
        &self,
        row: &mut AgentSessionRow,
        exit: RunExit,
    ) -> Result<Execution, AgentError> {
        self.store().set_agent_session_exit(&row.id, &exit)?;
        lock(&self.inner.tokens).remove(&row.id);
        let execution = exited(&exit);
        row.exit = Some(exit);
        Ok(execution)
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

fn live(state: TerminalState) -> Execution {
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
