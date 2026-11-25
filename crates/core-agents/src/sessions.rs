//! Agent sessions (decision 0051): the service object that starts runs in terminals, keeps session rows, reads each
//! session's execution from its terminal, and keeps each session's status (0052, 0053).
//!
//! Calls block (store, process spawn), so the service runs them on its blocking pool.
//!
//! Every status change (a start, a hook report, an exit, a forget) is applied under one lock, written to the session
//! row, and handed to the change listener in that same order.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, Weak};
use std::thread;
use std::time::{Duration, Instant};

use crate::credentials;
use massorca_core_store::{
    AgentSessionRow, RunExit, RunExitReason, RunMode, Store, StoreError, StoredActivity, new_id,
    now_ms,
};
use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::{
    CreateSpec, ExitReason as CoreExitReason, ExitRecord, ShellChoice, TerminalError,
    TerminalManager, TerminalState as CoreState,
};
use massorca_core_workspace::{DomainError, Workspaces};
use massorca_protocol::methods::agents::{
    AccountsChanged, Activity, ActivityState, AgentHookParams, AgentKinds, AgentModeSet,
    AgentSession, AgentStartParams, Execution, MAX_HOOK_PAYLOAD_BYTES, ModeApplied, ModeSource,
    PromptReceipt, ReceiptKind, ResumeState,
};
use massorca_protocol::methods::terminals::{
    DEFAULT_COLS, DEFAULT_ROWS, ExitReason, TerminalExit, TerminalState,
};
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceState};
use massorca_protocol::{ErrorBody, codes};
use serde_json::{Value, json};

use crate::catalog::Catalog;
use crate::kind::{KindDecl, PasteMode, PromptMode, StatusMechanism};
use crate::launch::{self, LaunchError, LaunchInput, LaunchPlan, Run};
use crate::modes;
use crate::packages::KindPackages;
use crate::paths;
use crate::prompt;
use crate::provided_env::{
    EnvProvider, PROVIDER_TIMEOUT, ProvidedEnv, SecretGrant, provide_bounded,
};
use crate::resolve::{SearchEnv, resolve_first};
use crate::resume_state::{self, ResumeInputs};
use crate::status::{self, HookRules};

mod delegation;
mod queue;

pub use delegation::{
    DELEGATION_DEPTH_DEFAULT, DELEGATION_DEPTH_MAX, DELEGATION_DEPTH_MIN, DELEGATION_DEPTH_SETTING,
    MergeResult, RepoOps,
};

/// How often the terminals of live runs are checked for an exit (0052).
pub const SWEEP_INTERVAL: Duration = Duration::from_millis(200);

/// How long a first prompt waits for its run's first `idle` before it is dropped (0054). The bound exists because an
/// agent that never reports idle would otherwise hold the text for as long as its run lives.
pub const FIRST_PROMPT_WAIT: Duration = Duration::from_secs(120);

/// The setting that names the folder conversations without a project are saved in (decision 0094), and its owner.
const CONVERSATIONS_SETTING: &str = "conversations.folder";
const CONVERSATIONS_OWNER: &str = "core";

/// The workspace and folder a no-project start made (decision 0094), removed again unless the start succeeds, so a
/// failed start leaves no empty conversation behind. The folder is removed only while it is still empty.
struct MadeConversation<'a> {
    agents: &'a Agents,
    workspace_id: String,
    folder: PathBuf,
}

impl MadeConversation<'_> {
    fn keep(&mut self) {
        self.workspace_id.clear();
    }
}

impl Drop for MadeConversation<'_> {
    fn drop(&mut self) {
        if self.workspace_id.is_empty() {
            return;
        }
        let _ = self.agents.store().delete_workspace(&self.workspace_id);
        let _ = std::fs::remove_dir(&self.folder);
    }
}

/// One applied status change, in the order the store applied it.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// The session's full record after the change.
    Updated(Box<AgentSession>),
    /// The session was forgotten; its id.
    Forgotten(String),
    /// An account of a kind changed, or its state did (decision 0059). Not a session change, so watchers do not get it.
    Account(AccountsChanged),
    /// A session's prompt queue changed; the session id (decision 0084). Not a session change, so watchers do not get it.
    Queue(String),
}

/// Receives every [`Change`]. Called under the status lock, so it must not block or call back into [`Agents`].
pub type Listener = Box<dyn Fn(Change) + Send + Sync>;

/// Shortest id prefix a session selector may be.
pub const MIN_SELECTOR_PREFIX: usize = 8;

/// The most output a release archives, in bytes: the newest of the terminal's text, cut on a character boundary (decision
/// 0090, the same bound as pv96's archive). The tail is the evidence, so the head goes first.
pub const ARCHIVE_MAX_BYTES: usize = 262_144;

/// The archived output of a released session, as its owner may read it (decision 0090).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivedOutput {
    pub text: String,
    pub truncated: bool,
}

/// The newest `max_bytes` of `text`, cut on a character boundary, and whether anything was cut.
fn keep_archive_tail(text: &str, max_bytes: usize) -> (String, bool) {
    if text.len() <= max_bytes {
        return (text.to_owned(), false);
    }
    let mut start = text.len() - max_bytes;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    (text[start..].to_owned(), true)
}

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
            LaunchError::ReservedName(_) => codes::AGENT_SECRETS_UNAVAILABLE,
            LaunchError::DangerousCallerEnv(_) => codes::REQUEST_INVALID_PARAMS,
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
    /// The conversations folder when `conversations.folder` is unset (decision 0094), `<home>/MassOrca/Conversations`.
    pub conversations_default: PathBuf,
    /// Variables each run receives from the profile's vault (decision 0071); `None` gives none.
    pub env_provider: Option<EnvProvider>,
    /// The packages that provide kinds and their state (decision 0059); [`crate::BuiltInPackages`] until the extension
    /// runtime provides them.
    pub kind_packages: Arc<dyn KindPackages>,
    /// The variables the vendor CLIs read, for the check that a session's saved conversation exists (decision 0160).
    /// `None` checks only account homes in the profile.
    pub vendor_env: Option<crate::VendorEnv>,
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
    /// The accounts a call is working on: sign-ins still open, and removals (decision 0059). See `accounts`.
    accounts: Mutex<crate::accounts::AccountBusy>,
    /// Serialises bring-backs, so a child is merged and marked once (0055).
    bring_back_gate: Mutex<()>,
    /// The workspaces a bring-back is merging into (0055). While one is listed, a start or resume in it and a prompt
    /// to one of its sessions without `force` are refused with `agent.busy`. Taken after the status lock when both are
    /// held.
    merging: Mutex<HashSet<String>>,
    /// How long a sign-in terminal may stay open with no result (decision 0059). Read from the test hook when the service
    /// starts; see [`crate::sign_in::SIGN_IN_TIMEOUT`].
    sign_in_timeout: Duration,
}

/// A run whose exit the service has not seen yet.
pub(crate) struct LiveRun {
    token: String,
    rules: HookRules,
    /// How a prompt is typed into the run (0054).
    prompt: PromptRules,
    /// A first prompt given at launch, held until the run's first `idle`.
    pending: Option<PendingPrompt>,
    /// The live switch of a mode with no launch arguments, typed at the run's first `idle` before the first prompt
    /// (decision 0083, D3).
    pending_switch: Option<PendingPrompt>,
    /// A prompt was typed since the run's last accepted hook report, so the next one waits for a report (decision
    /// 0058). Only a kind with a status mechanism sets it: nothing reports back otherwise.
    typed_since_report: bool,
    /// A mode switch is being typed into the run (decision 0083, R7): every other input waits for it, `force` or not, so
    /// two writes never interleave in the terminal.
    switching: bool,
    /// A queued prompt is being typed into the run (decision 0084): no other input waits for it, `force` or not.
    delivering: bool,
    /// The account the run uses, when its kind has one (decision 0059). A removal of that account is refused meanwhile.
    pub(crate) account: Option<String>,
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

pub(crate) type LiveRuns = HashMap<String, LiveRun>;

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
                accounts: Mutex::new(crate::accounts::AccountBusy::default()),
                bring_back_gate: Mutex::new(()),
                merging: Mutex::new(HashSet::new()),
                sign_in_timeout: crate::sign_in::sign_in_timeout_from(
                    std::env::var(crate::sign_in::SIGN_IN_TIMEOUT_ENV)
                        .ok()
                        .as_deref(),
                ),
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

    pub(crate) fn notify(&self, change: Change) {
        if let Some(listener) = self.inner.listener.get() {
            listener(change);
        }
    }

    /// The terminal manager the runs and sign-ins are created in.
    pub(crate) fn terminals(&self) -> &TerminalManager {
        &self.inner.config.terminals
    }

    /// How long a sign-in terminal may stay open with no result (decision 0059).
    pub(crate) fn sign_in_timeout(&self) -> Duration {
        self.inner.sign_in_timeout
    }

    /// The accounts a call is working on (sign-ins open, removals running), under the account busy lock.
    pub(crate) fn account_busy(&self) -> MutexGuard<'_, crate::accounts::AccountBusy> {
        lock(&self.inner.accounts)
    }

    /// The live runs, under the status lock.
    pub(crate) fn live_runs(&self) -> MutexGuard<'_, LiveRuns> {
        lock(&self.inner.status)
    }

    pub(crate) fn store(&self) -> &Store {
        &self.inner.config.store
    }

    pub(crate) fn profile_dir(&self) -> &Path {
        &self.inner.config.profile_dir
    }

    /// The packages that provide kinds (decision 0059).
    pub(crate) fn kind_packages(&self) -> &dyn KindPackages {
        self.inner.config.kind_packages.as_ref()
    }

    /// Every declared kind, read again from the profile, with programs resolved on the service's `PATH`.
    pub fn kinds(&self) -> AgentKinds {
        let profile_dir = self.profile_dir();
        let mut kinds = Catalog::load(self.kind_packages(), profile_dir)
            .views(profile_dir, &SearchEnv::current());
        // A kind is imported when its active account has a usable home (decision 0059).
        for kind in &mut kinds {
            kind.credentials.imported = self.has_usable_account(&kind.id);
        }
        AgentKinds { kinds }
    }

    /// The vault's variables for a run in `project_id`: none without a provider or a grant, and a failure once the
    /// provider takes longer than [`PROVIDER_TIMEOUT`].
    pub(crate) fn provided_env(
        &self,
        project_id: &str,
        grant: SecretGrant,
    ) -> Result<ProvidedEnv, AgentError> {
        let (Some(provider), SecretGrant::Provide) = (&self.inner.config.env_provider, grant)
        else {
            return Ok(ProvidedEnv::default());
        };
        provide_bounded(provider, project_id, PROVIDER_TIMEOUT).map_err(|error| {
            AgentError::new(
                codes::AGENT_SECRETS_UNAVAILABLE,
                format!("the run's secrets are unavailable: {error}"),
            )
        })
    }

    /// Starts a session: its first run in a new terminal in the workspace root (0051). The run receives the vault's
    /// variables only with [`SecretGrant::Provide`]; otherwise the session says why it has none.
    pub fn start(
        &self,
        params: AgentStartParams,
        grant: SecretGrant,
    ) -> Result<AgentSession, AgentError> {
        self.start_session(params, None, grant)
    }

    /// The workspace a start that names one runs in, once the start may use it: not being removed, not merged into, and
    /// its folder present.
    fn started_workspace(&self, selector: &str) -> Result<Workspace, AgentError> {
        let workspace = self.inner.config.workspaces.show_workspace(selector)?;
        refuse_removing(&workspace)?;
        self.refuse_while_merging(&workspace.id)?;
        if workspace.missing {
            return Err(AgentError::new(
                codes::PATH_NOT_FOUND,
                format!("the workspace directory is missing: {}", workspace.path),
            ));
        }
        Ok(workspace)
    }

    /// The folder workspace of a session without a project (decision 0094): its own folder, named after the session, in
    /// the conversations folder.
    fn conversation_workspace(&self, session_id: &str) -> Result<Workspace, AgentError> {
        let root = self.conversations_root()?;
        Ok(self
            .inner
            .config
            .workspaces
            .conversation_workspace(&root, session_id)?)
    }

    /// The conversations folder: the stored `conversations.folder`, else the default. It must be an absolute path.
    fn conversations_root(&self) -> Result<PathBuf, AgentError> {
        let stored = self
            .store()
            .get_setting(CONVERSATIONS_OWNER, CONVERSATIONS_SETTING)?;
        let root = match stored {
            Some(text) => serde_json::from_str::<String>(&text)
                .map(PathBuf::from)
                .map_err(|_| {
                    AgentError::new(
                        codes::CONFIG_INVALID_VALUE,
                        "conversations.folder is not a path",
                    )
                })?,
            None => self.inner.config.conversations_default.clone(),
        };
        if !root.is_absolute() {
            return Err(AgentError::new(
                codes::CONFIG_INVALID_VALUE,
                format!(
                    "conversations.folder must be an absolute path: {}",
                    root.display()
                ),
            ));
        }
        Ok(root)
    }

    /// Starts a session, delegated from `parent` when one is given (0055); a child of a run holding secrets is marked
    /// as holding them too.
    pub(crate) fn start_session(
        &self,
        params: AgentStartParams,
        parent: Option<&AgentSessionRow>,
        grant: SecretGrant,
    ) -> Result<AgentSession, AgentError> {
        let parent_holds = parent.is_some_and(|parent| parent.holds_secrets);
        let parent_id = parent.map(|parent| parent.id.clone());
        let _gate = lock(&self.inner.start_gate);
        if let Some(request_id) = &params.request_id
            && let Some(row) = self.store().find_agent_session_by_request_id(request_id)?
        {
            return self.view(row);
        }
        let chosen = match params.workspace.as_deref() {
            Some(selector) => Some(self.started_workspace(selector)?),
            None => None,
        };
        let (decl, program) = self.startable_kind(&params.kind)?;
        let mode = modes::plan_start(&decl, params.permission_mode.as_deref())?
            .with_caller_args(&params.args);
        // The model and effort the kind's declared flags take (decision 0094), after the mode's own arguments.
        let mode_args = [
            mode.args.clone(),
            decl.option_args(params.model.as_deref(), params.effort.as_deref())?,
        ]
        .concat();
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
        let session_id = new_id();
        // A session without a project gets its own folder once the start is valid (decision 0094).
        let (workspace, mut made) = match chosen {
            Some(workspace) => (workspace, None),
            None => {
                let workspace = self.conversation_workspace(&session_id)?;
                let made = MadeConversation {
                    agents: self,
                    workspace_id: workspace.id.clone(),
                    folder: PathBuf::from(&workspace.path),
                };
                (workspace, Some(made))
            }
        };
        if !params.read_only
            && !self.inner.config.workspaces.shares_writers(&workspace.id)?
            && let Some(holder) = self.live_writer(&workspace.id)?
        {
            let mut error = AgentError::new(
                codes::AGENT_WORKSPACE_BUSY,
                format!("session {holder} already writes in this workspace"),
            );
            error.data = Some(json!({ "session": holder }));
            return Err(error);
        }

        // Before the homes gate and the status lock: the provider reads the vault, bounded by its timeout.
        let provided = self.provided_env(&workspace.project_id, grant)?;
        let holds_secrets = !provided.is_empty() || parent_holds;
        let token = new_token();
        let new_ref = uuid::Uuid::new_v4().to_string();
        // The gate holds from the account's resolution until the run is registered, so a removal of the account sees it.
        let gate = credentials::lock_homes();
        let access = self.resolve_home(&gate, &decl, &workspace.project_id)?;
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
            credential_home: access.home.as_deref(),
            mode_args: &mode_args,
            caller_args: &params.args,
            provided_env: &provided,
            holds_secrets,
            caller_env: &params.env,
        })?;

        let run_dir = paths::run_dir(self.profile_dir(), &session_id);
        // The start is recorded before its program starts and settled once the row exists, so a crash in between leaves
        // a mark the next service start finds (docs/architecture/agents.md, "Crash-safe start").
        mark_starting(&run_dir).inspect_err(|_| remove_dir(&run_dir))?;
        // What the resume state reads later (decision 0160): the account this run uses, and no resume to classify.
        resume_state::record_run_account(&run_dir, access.account.as_deref())
            .and_then(|()| resume_state::record_probe(&run_dir, &decl, None))
            .inspect_err(|_| remove_dir(&run_dir))?;
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
                pending_switch: mode.switch_at_idle.as_deref().map(PendingPrompt::new),
                typed_since_report: false,
                switching: false,
                delivering: false,
                account: access.account.clone(),
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
        crash_point("agent-start-spawned");

        let row = AgentSessionRow {
            id: session_id.clone(),
            kind: decl.id.clone(),
            workspace_id: workspace.id,
            parent_id,
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
            holds_secrets,
            mode: mode.mode,
            pending_permission_mode: None,
            retained: false,
            released_at: None,
        };
        if let Err(error) = self.store().insert_agent_session(&row) {
            live.remove(&session_id);
            // Closed once the status lock is released: a close can wait for the terminal's kill grace.
            drop(live);
            let _ = self.inner.config.terminals.close(&terminal_id);
            remove_dir(&run_dir);
            return Err(error.into());
        }
        clear_starting(&run_dir);
        // The session row now owns its conversation folder.
        if let Some(made) = made.as_mut() {
            made.keep();
        }
        let mut session = self.view_with(&mut live, row)?;
        self.notify(Change::Updated(Box::new(session.clone())));
        session.secrets_withheld = withheld(grant);
        Ok(session)
    }

    /// Starts a new run of an exited session, in the same session (0054): the kind's resume arguments, with the agent's
    /// ref in place of `{ref}`, in a new terminal in the workspace root. The session keeps its id and its ref; its exit
    /// and activity are cleared, so the activity starts absent until the agent reports. Refused with `agent.busy` while
    /// its run is live; `agent.resume_unavailable` when the kind is no longer declared, has no resume form, or the
    /// agent's ref is not known; `agent.not_installed` when no program of the kind resolves; `agent.workspace_busy` when
    /// another session writes in the workspace.
    /// The run's permission mode is `permission_mode`, or the one decision 0083 (D6) keeps: a mode `set_mode` stored for
    /// this resume, else the last one unless it is a bypass, else the kind's default; an unknown id is
    /// `agent.mode_unknown`.
    pub fn resume(
        &self,
        selector: &str,
        permission_mode: Option<&str>,
        grant: SecretGrant,
    ) -> Result<AgentSession, AgentError> {
        self.resume_at(selector, permission_mode, grant, None, None)
    }

    /// `resume` with the terminal at `cols` x `rows`, each the default when absent: the size of the view that shows it.
    pub fn resume_at(
        &self,
        selector: &str,
        permission_mode: Option<&str>,
        grant: SecretGrant,
        cols: Option<u16>,
        rows: Option<u16>,
    ) -> Result<AgentSession, AgentError> {
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
        // The CLI saved no conversation for the ref, or a resume already found none (decision 0160): refused before a run
        // that would only fail.
        let state = resume_state::resume_state(&ResumeInputs {
            decl: Ok(&decl),
            agent_ref: Some(&agent_ref),
            exited: true,
            profile_dir: self.profile_dir(),
            session_id: &row.id,
            vendor_env: self.inner.config.vendor_env.as_ref(),
        });
        if let (false, Some(code)) = (state.available, state.code.as_deref())
            && code == codes::AGENT_RESUME_NOT_FOUND
        {
            return Err(AgentError::new(
                codes::AGENT_RESUME_NOT_FOUND,
                format!(
                    "session {} cannot resume: {}",
                    row.id,
                    state
                        .reason
                        .as_deref()
                        .unwrap_or(resume_state::NOT_FOUND_REASON)
                ),
            ));
        }
        let (mode, still_pending) = modes::plan_resume(
            &decl,
            permission_mode,
            &row.mode,
            row.pending_permission_mode.as_deref(),
        )?;
        let workspace = self
            .inner
            .config
            .workspaces
            .show_workspace(&row.workspace_id)?;
        refuse_removing(&workspace)?;
        self.refuse_while_merging(&workspace.id)?;
        if workspace.missing {
            return Err(AgentError::new(
                codes::PATH_NOT_FOUND,
                format!("the workspace directory is missing: {}", workspace.path),
            ));
        }
        if !row.read_only
            && !self.inner.config.workspaces.shares_writers(&workspace.id)?
            && let Some(holder) = self.live_writer(&workspace.id)?
        {
            let mut error = AgentError::new(
                codes::AGENT_WORKSPACE_BUSY,
                format!("session {holder} already writes in this workspace"),
            );
            error.data = Some(json!({ "session": holder }));
            return Err(error);
        }

        // Before the homes gate and the status lock, as in `start`.
        let provided = self.provided_env(&workspace.project_id, grant)?;
        let parent_holds = match &row.parent_id {
            Some(parent) => self
                .store()
                .get_agent_session(parent)?
                .is_some_and(|parent| parent.holds_secrets),
            None => false,
        };
        // Sticky: once a run of the session got secrets, its scrollback may hold them, so the mark never clears.
        let holds_secrets = !provided.is_empty() || parent_holds || row.holds_secrets;
        let token = new_token();
        // The gate holds from the account's resolution until the run is registered (see `start`).
        let gate = credentials::lock_homes();
        let access = self.resolve_home(&gate, &decl, &workspace.project_id)?;
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
            credential_home: access.home.as_deref(),
            mode_args: &mode.args,
            caller_args: &[],
            provided_env: &provided,
            holds_secrets,
            caller_env: &no_env,
        })?;
        let run_dir = paths::run_dir(self.profile_dir(), &row.id);
        // As in `start`: recorded before the program starts, settled once the row names the new terminal.
        mark_starting(&run_dir).inspect_err(|_| remove_dir(&run_dir))?;
        // The account this run uses, and the ref whose conversation its exit may show missing (decision 0160). The run
        // directory holds the earlier run's files, so it is kept on a failure here.
        resume_state::record_run_account(&run_dir, access.account.as_deref())?;
        resume_state::record_probe(&run_dir, &decl, Some(&agent_ref))?;
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
                pending_switch: mode.switch_at_idle.as_deref().map(PendingPrompt::new),
                typed_since_report: false,
                switching: false,
                delivering: false,
                account: access.account.clone(),
            },
        );
        let title = row.title.clone().unwrap_or_else(|| decl.name.clone());
        let size = PtySize {
            cols: cols.unwrap_or(DEFAULT_COLS),
            rows: rows.unwrap_or(DEFAULT_ROWS),
        };
        let terminal_id = self
            .spawn_run(plan, &workspace.id, &workspace.path, title, size)
            .inspect_err(|_| {
                live.remove(&row.id);
                remove_dir(&run_dir);
            })?;
        crash_point("agent-resume-spawned");
        let restarted = self.store().restart_agent_session(
            &row.id,
            &terminal_id,
            holds_secrets,
            &mode.mode,
            still_pending.as_deref(),
        );
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
        clear_starting(&run_dir);
        if let Some(previous) = row.terminal_id.as_deref().filter(|old| *old != terminal_id) {
            // The previous run's terminal is no longer the session's: unpinned, it is evicted like any exited terminal.
            let _ = self.inner.config.terminals.pin(previous, false);
        }
        let fresh = self.reload(&row.id)?;
        if fresh.retained {
            // A retained session keeps every run's terminal from eviction, not only the first (decision 0090).
            let _ = self.inner.config.terminals.pin(&terminal_id, true);
        }
        let mut session = self.view_with(&mut live, fresh)?;
        self.notify(Change::Updated(Box::new(session.clone())));
        session.secrets_withheld = withheld(grant);
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
            holds_secrets: plan.holds_secrets,
        };
        Ok(self.inner.config.terminals.create(spec)?.id)
    }

    /// The declaration and program a resume starts with: declared, valid, with a resume form, and installed (0054).
    fn resumable_kind(&self, row: &AgentSessionRow) -> Result<(KindDecl, PathBuf), AgentError> {
        let catalog = Catalog::load(self.kind_packages(), self.profile_dir());
        let entry = catalog.find(&row.kind).ok_or_else(|| {
            resume_unavailable(&row.id, &format!("kind {} is no longer declared", row.kind))
        })?;
        require_enabled(entry)?;
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
        // An idle or done agent takes the head of its queue (decision 0084, D3).
        let ready = matches!(report.activity.state.as_str(), "idle" | "done");
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
        // A mode's live switch held for this idle goes first, so the prompt runs in the mode (decision 0083, D3). While it
        // is typed the run is marked as switching, as `set_mode` marks it, so a prompt or switch meanwhile is refused.
        let has_terminal = session.terminal_id.is_some();
        let first = if first_idle {
            live.get_mut(&params.session).and_then(|run| {
                let switch = run.pending_switch.take().map(|switch| {
                    prompt::input_bytes(&switch.text, PasteMode::Plain, &run.prompt.submit)
                });
                let switching = switch.is_some() && has_terminal;
                if switching {
                    run.switching = true;
                }
                let text = run.pending.take().map(|pending| {
                    prompt::input_bytes(&pending.text, run.prompt.paste, &run.prompt.submit)
                });
                // Only a prompt counts as typed (decision 0058): a mode switch starts no turn for a report to answer.
                if text.is_some() {
                    run.typed_since_report = true;
                }
                let chunks: Vec<Vec<u8>> = switch.into_iter().chain(text).collect();
                (!chunks.is_empty()).then_some((chunks, switching))
            })
        } else {
            None
        };
        let terminal = session.terminal_id.clone();
        self.notify(Change::Updated(Box::new(session)));
        drop(live);
        if let (Some((chunks, switching)), Some(terminal)) = (first, terminal) {
            self.deliver_first_input(&params.session, &terminal, chunks, switching);
        }
        if ready {
            self.drain_queue(&params.session);
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
        self.send_prompt(selector, text, force, true)
    }

    /// The typing [`Agents::prompt`] does. `lift_queue_pauses` says the person sent it, which lifts the session's queue
    /// pauses; a system prompt, such as a bring-back's, does not (decision 0084, D5).
    pub(crate) fn send_prompt(
        &self,
        selector: &str,
        text: &str,
        force: bool,
        lift_queue_pauses: bool,
    ) -> Result<PromptReceipt, AgentError> {
        launch::check_prompt(text)?;
        let row = self.resolve(selector)?;
        // The readiness check and the copy of the input rules happen under the status lock. The write does not: it can
        // wait for the agent to read, and the agent's own hook reports need the lock to return.
        let (session_id, terminal, bytes) = {
            let mut live = lock(&self.inner.status);
            let row = self.reload(&row.id)?;
            let session = self.view_with(&mut live, row)?;
            // A bring-back merging into the workspace holds it until the merge ends (0055); `force` does not override
            // this, because a prompt could make the agent write into the worktree while git merges into it.
            self.refuse_while_merging(&session.workspace_id)?;
            let Some(run) = live.get_mut(&session.id) else {
                return Err(not_live(&session.id));
            };
            let state = session.activity.as_ref().map(|activity| activity.state);
            if let Some(refusal) = input_refusal(run, &session.id, state, force) {
                return Err(refusal);
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
        // An accepted prompt from the person lifts the session's queue pauses (decision 0084, D5).
        if lift_queue_pauses {
            self.lift_pauses(&session_id);
        }
        Ok(PromptReceipt {
            receipt: ReceiptKind::HandedToTerminal,
            bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        })
    }

    /// Switches a session's permission mode (decision 0083). When the session's run is live and the mode declares a live
    /// switch, the switch is typed into the run's terminal and the mode is recorded as chosen but unconfirmed: nothing
    /// confirms the agent took it (`ModeApplied::Live`). Otherwise the mode waits as `pending_permission_mode` for the next
    /// resume, and the running mode is left as it is (`ModeApplied::NextResume`); a pending bypass mode is used only by a
    /// resume that names it. A live switch waits for the run to be ready, as a prompt does (`agent.busy`). A mode a resume
    /// could not apply is refused with `agent.mode_unavailable`, a session its kind cannot resume with
    /// `agent.resume_unavailable`, and an unknown mode with `agent.mode_unknown`.
    ///
    /// A kind with no status mechanism cannot say when its agent is between prompts, so a live switch into it needs
    /// `force` (R8); without it the call is refused with `agent.mode_unavailable`. While the switch is typed the run is
    /// marked as switching, and every other prompt or switch is refused with `agent.busy` (R7).
    pub fn set_mode(
        &self,
        selector: &str,
        mode: &str,
        force: bool,
    ) -> Result<AgentModeSet, AgentError> {
        let row = self.resolve(selector)?;
        let decl = self.declared_kind(&row.kind)?;
        let declared = modes::declared(&decl, mode)?;
        let live_text = declared
            .live_switch
            .as_ref()
            .map(|switch| switch.text.clone());
        let typed = {
            let mut live = lock(&self.inner.status);
            let row = self.reload(&row.id)?;
            let session = self.view_with(&mut live, row)?;
            let switchable = live.contains_key(&session.id) && session.terminal_id.is_some();
            match (switchable, live_text) {
                (true, Some(text)) => {
                    let terminal = session.terminal_id.clone().unwrap_or_default();
                    let Some(run) = live.get_mut(&session.id) else {
                        return Err(not_live(&session.id));
                    };
                    let state = session.activity.as_ref().map(|activity| activity.state);
                    if let Some(refusal) = input_refusal(run, &session.id, state, false) {
                        return Err(refusal);
                    }
                    if !run.prompt.has_status && !force {
                        return Err(AgentError::new(
                            codes::AGENT_MODE_UNAVAILABLE,
                            format!(
                                "kind {} reports no activity, so MassOrca cannot tell whether session {} is in the middle of a prompt, and typing {mode}'s switch now could land inside it; switch with force (agent mode --force) once you know the agent is waiting for input",
                                decl.id, session.id
                            ),
                        ));
                    }
                    run.switching = true;
                    // A mode switch starts no turn, so no report answers it: it leaves the typed-since-report flag
                    // alone, or the next prompt would wait for a report that never comes.
                    let bytes = prompt::input_bytes(&text, PasteMode::Plain, &run.prompt.submit);
                    Some((terminal, bytes))
                }
                _ => {
                    modes::applies_at_resume(&decl, declared)?;
                    if !decl.resumable() {
                        return Err(resume_unavailable(
                            &session.id,
                            &format!(
                                "kind {} cannot resume it and has no live switch to mode {mode} for this run",
                                decl.id
                            ),
                        ));
                    }
                    self.store()
                        .set_agent_session_pending_mode(&session.id, Some(mode))?;
                    let session = self.view_with(&mut live, self.reload(&session.id)?)?;
                    self.notify(Change::Updated(Box::new(session.clone())));
                    return Ok(AgentModeSet {
                        applied: ModeApplied::NextResume,
                        session,
                    });
                }
            }
        };
        // Typed outside the status lock, as a prompt is: the write can wait for the agent to read. The run stays marked as
        // switching until the write is done, whatever its outcome.
        let written =
            typed.map(|(terminal, bytes)| self.inner.config.terminals.input(&terminal, &bytes));
        self.end_switching(&row.id);
        // The switch is typed and the agent is as ready as it was: the queue may go on (decision 0084, D3).
        self.drain_queue(&row.id);
        let mut live = lock(&self.inner.status);
        if let Some(Err(error)) = written {
            return Err(match error {
                TerminalError::Exited | TerminalError::NotFound => not_live(&row.id),
                other => AgentError::from(other),
            });
        }
        let switched = RunMode {
            permission_mode: Some(mode.to_owned()),
            source: Some(modes::SOURCE_CHOSEN.to_owned()),
            confirmed: false,
        };
        self.store().set_agent_session_mode(&row.id, &switched)?;
        let session = self.view_with(&mut live, self.reload(&row.id)?)?;
        self.notify(Change::Updated(Box::new(session.clone())));
        Ok(AgentModeSet {
            applied: ModeApplied::Live,
            session,
        })
    }

    /// The first bypass mode (decision 0083) that kind `id` declares and a new run of it can apply: one with launch
    /// arguments, or a live switch on a kind that reports idle. `None` when it declares none, or none a start can apply.
    pub fn startable_bypass_mode(&self, id: &str) -> Option<String> {
        let decl = self.declared_kind(id).ok()?;
        decl.permission_modes
            .iter()
            .find(|mode| mode.bypass && modes::plan(&decl, mode).is_ok())
            .map(|mode| mode.id.clone())
    }

    /// The declaration of `id`: declared and valid, whether or not its program is installed now.
    fn declared_kind(&self, id: &str) -> Result<KindDecl, AgentError> {
        let catalog = Catalog::load(self.kind_packages(), self.profile_dir());
        let entry = catalog.find(id).ok_or_else(|| {
            AgentError::new(codes::AGENT_KIND_NOT_FOUND, format!("no agent kind {id}"))
        })?;
        entry.valid().cloned().ok_or_else(|| {
            AgentError::new(
                codes::AGENT_NOT_INSTALLED,
                format!("agent kind {id} is invalid: {}", entry.problems.join("; ")),
            )
        })
    }

    /// Types what a run held for its first `idle` (a mode's live switch, then the first prompt) on a thread of its own,
    /// in that order. The write can wait for the agent to read, and the agent waits for the hook report that carried this
    /// `idle` to return, so the report must not wait for the write (0054).
    /// The run's switching mark, if the first idle set one, is cleared once the writes are done, whatever their outcome.
    fn deliver_first_input(
        &self,
        session: &str,
        terminal: &str,
        chunks: Vec<Vec<u8>>,
        switching: bool,
    ) {
        let agents = self.clone();
        let terminal = terminal.to_owned();
        let owner = session.to_owned();
        let spawned = thread::Builder::new()
            .name("agent-first-prompt".to_owned())
            .spawn(move || {
                for bytes in chunks {
                    if let Err(error) = agents.inner.config.terminals.input(&terminal, &bytes) {
                        let error = AgentError::from(error);
                        tracing::warn!(
                            session = %owner,
                            code = error.code,
                            "the first input could not be typed into the run's terminal"
                        );
                        break;
                    }
                }
                if switching {
                    agents.end_switching(&owner);
                    agents.drain_queue(&owner);
                }
            });
        if let Err(error) = spawned {
            tracing::warn!(session = %session, %error, "the first prompt has no thread to be typed on");
            if switching {
                self.end_switching(session);
            }
        }
    }

    /// Clears the run's switching mark (decision 0083, R7).
    fn end_switching(&self, session: &str) {
        if let Some(run) = lock(&self.inner.status).get_mut(session) {
            run.switching = false;
        }
    }

    /// Checks the terminal of every live run once and applies the exits it finds, in order with hook reports. Each
    /// session is checked on its own: one that fails is logged, by its id and error code only, and the sweep goes on.
    pub fn sweep(&self) {
        let mut live = lock(&self.inner.status);
        for id in expire_first_prompts(&mut live, Instant::now()) {
            tracing::warn!(
                session = %id,
                "the first input was dropped: the agent reported no idle state in time"
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

    /// Marks a session's current `waiting` or `done` as seen (inventory STAT-19) and announces the change once, so every
    /// reader clears its marker together. With `saw`, only the activity recorded at that time is marked. Any other
    /// activity, or one already seen, is returned as it is.
    pub fn mark_seen(&self, selector: &str, saw: Option<i64>) -> Result<AgentSession, AgentError> {
        let id = self.resolve(selector)?.id;
        let mut live = lock(&self.inner.status);
        let marked = self.store().mark_agent_session_seen(&id, saw, now_ms())?;
        let row = self.reload(&id)?;
        let session = self.view_with(&mut live, row)?;
        if marked {
            self.notify(Change::Updated(Box::new(session.clone())));
        }
        Ok(session)
    }

    /// Ends the current run (graceful, or at once with `force`). The record stays, exited with reason `killed`.
    /// The sessions delegated from it end too, the root first (0055): stopping a root stops its whole tree. A session
    /// whose run already ended is left as it is.
    pub fn stop(&self, selector: &str, force: bool) -> Result<AgentSession, AgentError> {
        let root = self.resolve(selector)?;
        for id in self.subtree(&root.id)? {
            self.stop_run(&id, force)?;
        }
        let row = self.reload(&root.id)?;
        self.view(row)
    }

    /// Ends the run of one session that has a live run, as [`Agents::stop`] does for each session in its tree.
    fn stop_run(&self, id: &str, force: bool) -> Result<(), AgentError> {
        let row = self.reload(id)?;
        // The person's stop pauses the queue before the run is killed, so a grace-period idle cannot deliver (decision 0084, D5).
        self.pause_queue_stopped(&row.id);
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
        Ok(())
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

    /// Releases an exited session (decision 0090): archives the newest output of its terminal, frees the terminal, clears
    /// its retain and keeps the record, marked released. A live run is refused with `agent.busy`. When the terminal's
    /// output cannot be read, the terminal is kept and the release fails with `agent.archive_failed`. A terminal the
    /// manager no longer has has no output left, so its archive is empty.
    pub fn release(&self, selector: &str) -> Result<AgentSession, AgentError> {
        let id = self.resolve(selector)?.id;
        let mut live = lock(&self.inner.status);
        let row = self.reload(&id)?;
        let current = self.view_with(&mut live, row.clone())?;
        if current.execution.state != TerminalState::Exited {
            return Err(AgentError::new(
                codes::AGENT_BUSY,
                format!("session {} is still running; stop it first", current.id),
            ));
        }
        if row.released_at.is_some() {
            return Ok(current);
        }
        let (text, truncated) = match &row.terminal_id {
            Some(terminal) => match self.inner.config.terminals.text(terminal) {
                Ok(text) => keep_archive_tail(&text, ARCHIVE_MAX_BYTES),
                Err(TerminalError::NotFound) => (String::new(), false),
                Err(error) => {
                    return Err(AgentError::new(
                        codes::AGENT_ARCHIVE_FAILED,
                        format!(
                            "the output of session {id} could not be archived, so its terminal was kept: {error}"
                        ),
                    ));
                }
            },
            None => (String::new(), false),
        };
        if !self
            .store()
            .release_agent_session(&id, &text, truncated, now_ms())?
        {
            // Nothing was recorded, so the terminal is kept.
            return Err(AgentError::new(
                codes::AGENT_BUSY,
                format!("session {id} has no recorded end yet; stop it first"),
            ));
        }
        let released = self.reload(&id)?;
        let session = self.view_with(&mut live, released)?;
        self.notify(Change::Updated(Box::new(session.clone())));
        // Closed once the status lock is released: a close can wait for the terminal's kill grace.
        drop(live);
        if let Some(terminal) = &row.terminal_id {
            let _ = self.inner.config.terminals.close(terminal);
        }
        Ok(session)
    }

    /// Sets or clears a session's retain (decision 0090). A retained session's exited terminal is kept from eviction: the
    /// terminal is pinned now and every later run's terminal is pinned when it starts. Retaining a released session only
    /// sets the flag, since its terminal is gone.
    pub fn set_retained(&self, selector: &str, retained: bool) -> Result<AgentSession, AgentError> {
        let id = self.resolve(selector)?.id;
        let mut live = lock(&self.inner.status);
        // Pinned before the flag is saved, so a failed pin leaves the session as it was.
        if let Some(terminal) = &self.reload(&id)?.terminal_id {
            match self.inner.config.terminals.pin(terminal, retained) {
                // A terminal the manager no longer has has nothing left to keep.
                Ok(()) | Err(TerminalError::NotFound) => {}
                Err(error) => return Err(error.into()),
            }
        }
        self.store().set_agent_session_retained(&id, retained)?;
        let row = self.reload(&id)?;
        let session = self.view_with(&mut live, row)?;
        self.notify(Change::Updated(Box::new(session.clone())));
        Ok(session)
    }

    /// A session and its archived output (decision 0090): `None` when the session was not released. The text is not
    /// redacted here; the service applies decision 0071 to it.
    pub fn archived_output(
        &self,
        selector: &str,
    ) -> Result<(AgentSession, Option<ArchivedOutput>), AgentError> {
        let id = self.resolve(selector)?.id;
        let session = self.show(&id)?;
        let output = self
            .store()
            .get_agent_session_archive(&id)?
            .map(|archive| ArchivedOutput {
                text: archive.text,
                truncated: archive.truncated,
            });
        Ok((session, output))
    }

    /// Marks every session whose run was live when the service last stopped as exited with reason `service_stopped`
    /// (0051). Called once at service start, before any run exists. Returns how many sessions changed.
    /// A run that was `working` or `waiting` then is `done` with outcome `interrupted` (0052).
    pub fn mark_stale_on_startup(&self) -> Result<u64, AgentError> {
        self.settle_interrupted_starts()?;
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
        // A restart pauses every waiting queued prompt, and fails the one being sent (decision 0084, D5).
        self.store().restart_queued_prompts(at)?;
        Ok(self.store().mark_agent_runs_service_stopped(at)?)
    }

    /// Refuses removing, disabling or updating away package `package_id` while a session of one of its kinds is live
    /// or resumable (an exited run with a recorded end and a known ref), with `agent.busy` naming those sessions. The
    /// extensions remove guard calls it (decision 0062); `agent forget` frees a session.
    pub fn refuse_removing_package(&self, package_id: &str) -> Result<(), AgentError> {
        let kinds: BTreeSet<String> = self
            .kind_packages()
            .kinds()
            .into_iter()
            .filter(|kind| kind.package_id == package_id)
            .map(|kind| kind.kind_id)
            .collect();
        if kinds.is_empty() {
            return Ok(());
        }
        let mut sessions = Vec::new();
        for row in self.store().list_agent_sessions(None)? {
            if !kinds.contains(&row.kind) {
                continue;
            }
            let resumable = row.exit.is_some() && row.agent_ref.is_some();
            let id = row.id.clone();
            let live = self.view(row)?.execution.state != TerminalState::Exited;
            if live || resumable {
                sessions.push(id);
            }
        }
        if sessions.is_empty() {
            return Ok(());
        }
        let names: Vec<&str> = kinds.iter().map(String::as_str).collect();
        let mut error = AgentError::new(
            codes::AGENT_BUSY,
            format!(
                "package {package_id} provides agent kind {}, which sessions {} still use; stop and forget them first",
                names.join(", "),
                sessions.join(", ")
            ),
        );
        error.data = Some(json!({ "package": package_id, "kinds": names, "sessions": sessions }));
        Err(error)
    }

    /// Settles the starts the previous service recorded and never settled (docs/architecture/agents.md, "Crash-safe
    /// start"). A marked run directory with no session row is a start that died before its row existed: its program
    /// ended with that service (0051), so the directory, which may hold the run's hook token and launch files, is
    /// deleted and a retry of the same request starts afresh. A marked directory whose row exists is a start or resume
    /// that died after it was recorded; the row is marked stale like any other, so only the mark is removed.
    fn settle_interrupted_starts(&self) -> Result<(), AgentError> {
        let runs = paths::agents_dir(self.profile_dir()).join("runs");
        let entries = match std::fs::read_dir(&runs) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.join(STARTING_MARK).is_file() {
                continue;
            }
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if self.store().get_agent_session(&id)?.is_some() {
                clear_starting(&dir);
            } else {
                tracing::warn!(session = %id, "a start the previous service did not finish is cleaned up");
                remove_dir(&dir);
            }
        }
        Ok(())
    }

    /// The declaration and program a start uses: declared, enabled (decision 0059), valid and installed. A delegated
    /// child and a sign-in start through it too.
    pub(crate) fn startable_kind(
        &self,
        id: &str,
    ) -> Result<(crate::KindDecl, PathBuf), AgentError> {
        self.installed_kind(id, true)
    }

    /// [`Agents::startable_kind`], whatever the state of the kind's package: an account's check, its removal and its
    /// sign-out keep working on a disabled kind (decision 0059).
    pub(crate) fn runnable_kind(&self, id: &str) -> Result<(crate::KindDecl, PathBuf), AgentError> {
        self.installed_kind(id, false)
    }

    fn installed_kind(
        &self,
        id: &str,
        gated: bool,
    ) -> Result<(crate::KindDecl, PathBuf), AgentError> {
        let catalog = Catalog::load(self.kind_packages(), self.profile_dir());
        let entry = catalog.find(id).ok_or_else(|| {
            AgentError::new(codes::AGENT_KIND_NOT_FOUND, format!("no agent kind {id}"))
        })?;
        if gated {
            require_enabled(entry)?;
        }
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

    /// Refuses `kind` with `agent.kind_not_enabled` when it is declared by a package that is not enabled (decision
    /// 0059). An undeclared kind passes: the call reports it as its own.
    pub(crate) fn require_enabled_kind(&self, kind: &str) -> Result<(), AgentError> {
        match Catalog::load(self.kind_packages(), self.profile_dir()).find(kind) {
            Some(entry) => require_enabled(entry),
            None => Ok(()),
        }
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
        let resume = self.resume_of(&row, execution.state == TerminalState::Exited);
        Ok(record(row, execution, resume))
    }

    /// The session's resume state (decision 0160), from its kind as declared now.
    fn resume_of(&self, row: &AgentSessionRow, exited: bool) -> ResumeState {
        let catalog = Catalog::load(self.kind_packages(), self.profile_dir());
        let decl = match catalog.find(&row.kind) {
            None => Err(format!("kind {} is no longer declared", row.kind)),
            Some(entry) => entry
                .valid()
                .ok_or_else(|| format!("kind {} is invalid", row.kind)),
        };
        resume_state::resume_state(&ResumeInputs {
            decl,
            agent_ref: row.agent_ref.as_deref(),
            exited,
            profile_dir: self.profile_dir(),
            session_id: &row.id,
            vendor_env: self.inner.config.vendor_env.as_ref(),
        })
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
            // A resumed run that failed because its CLI found no conversation leaves the session unable to resume.
            let catalog = Catalog::load(self.kind_packages(), self.profile_dir());
            let decl = catalog.find(&row.kind).and_then(|entry| entry.valid());
            let failed = exit.code.is_some_and(|code| code != 0);
            resume_state::classify_exit(
                &paths::run_dir(self.profile_dir(), &row.id),
                decl,
                failed,
                || self.inner.config.terminals.text(terminal).ok(),
            );
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
            let resume = self.resume_of(row, true);
            self.notify(Change::Updated(Box::new(record(
                row.clone(),
                execution.clone(),
                resume,
            ))));
        }
        Ok(execution)
    }
}

/// The wire record of a row with its execution.
fn record(row: AgentSessionRow, execution: Execution, resume: ResumeState) -> AgentSession {
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
        holds_secrets: row.holds_secrets,
        secrets_withheld: None,
        permission_mode: row.mode.permission_mode,
        mode_confirmed: row.mode.confirmed,
        retained: row.retained,
        released: row.released_at.is_some(),
        mode_source: row.mode.source.as_deref().map(mode_source_of),
        pending_permission_mode: row.pending_permission_mode,
        resume: Some(resume),
    }
}

/// Why `run` takes no input now, if it does not (decisions 0058, 0083): a mode switch is being typed into it, whatever
/// `force` says (R7); or, without `force`, its agent has not reported ready, or has not answered the last prompt yet, so
/// a second one would land on the same idle.
fn input_refusal(
    run: &LiveRun,
    session: &str,
    state: Option<ActivityState>,
    force: bool,
) -> Option<AgentError> {
    if run.delivering {
        return Some(AgentError::new(
            codes::AGENT_BUSY,
            format!("session {session} is typing a queued prompt; try again once it is typed"),
        ));
    }
    if run.switching {
        return Some(AgentError::new(
            codes::AGENT_BUSY,
            format!(
                "session {session} is switching its permission mode; try again once the switch is typed"
            ),
        ));
    }
    if !force && (!prompt::ready(run.prompt.has_status, state) || run.typed_since_report) {
        return Some(AgentError::new(
            codes::AGENT_BUSY,
            format!(
                "session {session} is not ready for a prompt: wait for its agent to report, or use force"
            ),
        ));
    }
    None
}

/// A stored mode source in wire form; a value a newer build wrote reads as `unknown`.
fn mode_source_of(stored: &str) -> ModeSource {
    match stored {
        modes::SOURCE_VENDOR_DEFAULT => ModeSource::VendorDefault,
        modes::SOURCE_CHOSEN => ModeSource::Chosen,
        _ => ModeSource::Unknown,
    }
}

/// Why a run started without the vault's variables, for its caller.
fn withheld(grant: SecretGrant) -> Option<String> {
    match grant {
        SecretGrant::Provide => None,
        SecretGrant::Withhold(reason) => Some(reason.to_owned()),
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
        let mut dropped = false;
        for held in [&mut run.pending, &mut run.pending_switch] {
            if held.as_ref().is_some_and(|pending| now >= pending.due) {
                *held = None;
                dropped = true;
            }
        }
        if dropped {
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

/// The file in a run's directory that records a start or resume in progress: written before its program starts,
/// removed once the session's row records the run (docs/architecture/agents.md, "Crash-safe start").
const STARTING_MARK: &str = "starting";

fn mark_starting(run_dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(run_dir)?;
    std::fs::write(run_dir.join(STARTING_MARK), b"")
}

fn clear_starting(run_dir: &Path) {
    let _ = std::fs::remove_file(run_dir.join(STARTING_MARK));
}

/// Test-only: names a point where the service exits at once, as a crash there would. Compiled in only with the
/// `test-hooks` feature, which only test builds enable.
#[cfg(feature = "test-hooks")]
const CRASH_AT_ENV: &str = "MASSORCA_TEST_CRASH_AT";

/// Exits the process when [`CRASH_AT_ENV`] names `point`, with no cleanup, as a crash would. Without the `test-hooks`
/// feature it does nothing and reads no variable.
#[cfg(feature = "test-hooks")]
fn crash_point(point: &str) {
    if std::env::var_os(CRASH_AT_ENV).is_some_and(|value| value == point) {
        tracing::error!(point, "exiting at a test crash point");
        std::process::exit(70);
    }
}

#[cfg(not(feature = "test-hooks"))]
fn crash_point(_point: &str) {}

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

/// Refuses an entry whose package is not enabled (decision 0059), saying in plain words which agent it is and why; the
/// code and data still name the kind, its package and the state. A state that cannot be read is reported as that.
fn require_enabled(entry: &crate::CatalogEntry) -> Result<(), AgentError> {
    if entry.enabled() {
        return Ok(());
    }
    let state = serde_json::to_value(entry.package.state)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned());
    let package = entry.package.id.clone().unwrap_or_default();
    let name = entry
        .decl
        .as_ref()
        .map_or(entry.id.as_str(), |decl| decl.name.as_str());
    // A state that cannot be read (an unreadable lockfile) is that failure, which installing or enabling cannot fix.
    let message = if state == "unknown" && !entry.problems.is_empty() {
        format!(
            "agent kind {} comes from package {package}, whose state cannot be read: {}",
            entry.id,
            entry.problems.join("; ")
        )
    } else {
        let reason = match state.as_str() {
            "available" => format!("{name} is not installed yet"),
            "disabled" => format!("{name} is turned off"),
            _ => format!("{name} is not ready to use"),
        };
        format!("{reason}; install or enable it in the Marketplace.")
    };
    let mut error = AgentError::new(codes::AGENT_KIND_NOT_ENABLED, message);
    error.data = Some(json!({ "kind": entry.id, "package": package, "state": state }));
    Err(error)
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
    if let Some(seen_at) = stored.seen_at {
        value["seen_at_ms"] = json!(seen_at);
    }
    serde_json::from_value(value).ok()
}

/// No run starts in a workspace being removed: its removal stops the runs it finds, and must find them all.
fn refuse_removing(workspace: &Workspace) -> Result<(), AgentError> {
    if workspace.state == WorkspaceState::Removing {
        return Err(AgentError::new(
            codes::WORKSPACE_NOT_FOUND,
            format!("workspace `{}` is being removed", workspace.name),
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use massorca_core_git::Git;
    use massorca_core_store::{
        ProjectKind, ProjectRow, WorkspaceKind, WorkspaceRow, WorkspaceState,
    };
    use massorca_core_terminal::ManagerConfig;
    use massorca_core_terminal::shell::EnvSnapshot;
    use massorca_protocol::methods::agents::MAX_PROMPT_BYTES;

    use super::*;

    /// A service over an in-memory store with one workspace; returns it, the workspace id and the profile directory.
    pub(crate) fn service() -> (Agents, String, tempfile::TempDir) {
        service_with(Arc::new(crate::BuiltInPackages))
    }

    /// [`service`], with kinds from `packages`.
    pub(crate) fn service_with(
        packages: Arc<dyn KindPackages>,
    ) -> (Agents, String, tempfile::TempDir) {
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
            base_ref: None,
            icon: None,
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
            comment: None,
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
            conversations_default: std::env::temp_dir().join("MassOrca").join("Conversations"),
            env_provider: None,
            kind_packages: packages,
            vendor_env: None,
        });
        (agents, workspace.id, profile)
    }

    fn activity(state: &str, outcome: Option<&str>) -> StoredActivity {
        StoredActivity {
            state: state.to_owned(),
            outcome: outcome.map(str::to_owned),
            source: "hook".to_owned(),
            at: 5,
            seen_at: None,
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
            holds_secrets: false,
            mode: RunMode::default(),
            pending_permission_mode: None,
            retained: false,
            released_at: None,
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
    /// The window a switch is being typed in is only as long as one terminal write, so a race with a real prompt is not
    /// deterministic; the rule that refuses one is tested here on its own instead.
    #[test]
    fn a_run_typing_a_mode_switch_refuses_every_other_input_even_forced() {
        let run = |switching| LiveRun {
            token: "t".to_owned(),
            rules: HookRules::default(),
            prompt: PromptRules {
                has_status: true,
                paste: PasteMode::Plain,
                submit: "\r".to_owned(),
            },
            pending: None,
            pending_switch: None,
            typed_since_report: false,
            switching,
            delivering: false,
            account: None,
        };
        let idle = Some(ActivityState::Idle);
        assert!(input_refusal(&run(false), "s", idle, false).is_none());
        for force in [false, true] {
            let refused = input_refusal(&run(true), "s", idle, force).expect("refused");
            assert_eq!(refused.code, codes::AGENT_BUSY);
            assert!(refused.message.contains("switching"), "{}", refused.message);
        }
        let working = Some(ActivityState::Working);
        assert!(input_refusal(&run(false), "s", working, false).is_some());
        assert!(
            input_refusal(&run(false), "s", working, true).is_none(),
            "force types anyway"
        );
    }

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
            pending_switch: Some(PendingPrompt {
                text: "/mode wild".to_owned(),
                due,
            }),
            typed_since_report: false,
            switching: false,
            delivering: false,
            account: None,
        };
        let mut live: LiveRuns = HashMap::new();
        live.insert("waiting".to_owned(), run(now + FIRST_PROMPT_WAIT));
        live.insert("late".to_owned(), run(now));

        assert_eq!(
            expire_first_prompts(&mut live, now),
            vec!["late".to_owned()]
        );
        assert!(live["waiting"].pending.is_some());
        assert!(live["waiting"].pending_switch.is_some());
        assert!(live["late"].pending.is_none());
        assert!(
            live["late"].pending_switch.is_none(),
            "a held mode switch expires with the prompt"
        );
    }

    /// A resume of a session that does not exist is refused as not found (0054).
    #[test]
    fn a_resume_of_an_unknown_session_is_not_found() {
        let (agents, _workspace_id, _profile) = service();
        let error = agents
            .resume("no-such-session", None, SecretGrant::Provide)
            .unwrap_err();
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
            holds_secrets: false,
            mode: RunMode::default(),
            pending_permission_mode: None,
            retained: false,
            released_at: None,
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
                .restart_agent_session(&row.id, "t-2", false, &RunMode::default(), None)
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
                pending_switch: None,
                typed_since_report: false,
                switching: false,
                delivering: false,
                account: None,
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
            holds_secrets: false,
            mode: RunMode::default(),
            pending_permission_mode: None,
            retained: false,
            released_at: None,
        };
        agents.store().insert_agent_session(&row).unwrap();

        let error = agents
            .resume(&row.id, None, SecretGrant::Provide)
            .unwrap_err();
        assert_eq!(error.code, codes::AGENT_RESUME_UNAVAILABLE, "{error}");
    }
}

#[cfg(test)]
mod release_tests {
    use super::tests::service;
    use super::*;

    /// An exited session of `workspace` whose terminal is `terminal`, which the terminal manager may not know.
    fn exited_row(workspace: &str, terminal: &str) -> AgentSessionRow {
        AgentSessionRow {
            id: new_id(),
            kind: "fake".to_owned(),
            workspace_id: workspace.to_owned(),
            parent_id: None,
            title: None,
            read_only: false,
            agent_ref: None,
            terminal_id: Some(terminal.to_owned()),
            request_id: None,
            created_at: now_ms(),
            exit: Some(RunExit {
                code: Some(0),
                signal: None,
                reason: RunExitReason::Exited,
                at: now_ms(),
            }),
            activity: None,
            result: None,
            brought_back_at: None,
            holds_secrets: false,
            mode: RunMode::default(),
            pending_permission_mode: None,
            retained: false,
            released_at: None,
        }
    }

    #[test]
    fn release_keeps_the_record_and_archives_an_empty_tail_when_the_terminal_is_gone() {
        let (agents, workspace, _profile) = service();
        let row = exited_row(&workspace, "gone");
        agents.store().insert_agent_session(&row).unwrap();

        let released = agents.release(&row.id).unwrap();
        assert!(released.released);
        assert_eq!(released.execution.state, TerminalState::Exited);
        let (session, output) = agents.archived_output(&row.id).unwrap();
        assert!(session.released);
        assert_eq!(
            output,
            Some(ArchivedOutput {
                text: String::new(),
                truncated: false,
            })
        );
        assert!(
            agents.release(&row.id).unwrap().released,
            "a second release changes nothing"
        );
    }

    #[test]
    fn retain_is_a_flag_that_a_release_clears() {
        let (agents, workspace, _profile) = service();
        let row = exited_row(&workspace, "gone");
        agents.store().insert_agent_session(&row).unwrap();

        assert!(agents.set_retained(&row.id, true).unwrap().retained);
        let released = agents.release(&row.id).unwrap();
        assert!(!released.retained, "an explicit release clears the retain");
        assert!(released.released);
        assert!(!agents.set_retained(&row.id, false).unwrap().retained);
    }

    #[test]
    fn the_archive_keeps_the_newest_bytes_on_a_char_boundary() {
        let text = format!("{}é{}", "x".repeat(10), "y".repeat(3));
        assert_eq!(keep_archive_tail("abc", 5), ("abc".to_owned(), false));
        assert_eq!(keep_archive_tail(&text, 6), ("xéyyy".to_owned(), true));
        assert_eq!(keep_archive_tail(&text, 5), ("éyyy".to_owned(), true));
        // Four bytes would start inside the two-byte é: the cut moves forward to the next character.
        assert_eq!(keep_archive_tail(&text, 4), ("yyy".to_owned(), true));
        assert_eq!(ARCHIVE_MAX_BYTES, 262_144);
    }
}

#[cfg(test)]
mod not_enabled_tests {
    use massorca_protocol::methods::agents::{KindPackage, PackageState};
    use serde_json::json;

    use super::*;
    use crate::CatalogEntry;

    fn entry(state: PackageState) -> CatalogEntry {
        CatalogEntry {
            id: "antigravity".to_owned(),
            owner: "massorca".to_owned(),
            decl: KindDecl::from_json(
                r#"{"id":"antigravity","name":"Antigravity","program":["agy"]}"#,
            )
            .ok(),
            source: None,
            problems: Vec::new(),
            package: KindPackage {
                state,
                id: Some("massorca.antigravity".to_owned()),
                version: None,
            },
        }
    }

    /// The refusal names the agent by its display name; its code and data are unchanged.
    #[test]
    fn a_kind_whose_package_is_not_installed_is_refused_by_name() {
        let error = require_enabled(&entry(PackageState::Available)).expect_err("refused");
        assert_eq!(error.code, codes::AGENT_KIND_NOT_ENABLED);
        assert_eq!(
            error.message,
            "Antigravity is not installed yet; install or enable it in the Marketplace."
        );
        assert_eq!(
            error.data,
            Some(
                json!({ "kind": "antigravity", "package": "massorca.antigravity", "state": "available" })
            )
        );
    }

    #[test]
    fn a_kind_whose_package_is_off_is_refused_by_name() {
        let error = require_enabled(&entry(PackageState::Disabled)).expect_err("refused");
        assert_eq!(
            error.message,
            "Antigravity is turned off; install or enable it in the Marketplace."
        );
    }
}
