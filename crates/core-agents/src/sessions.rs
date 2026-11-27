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
use crate::fast_mode;
use massorca_core_store::{
    AgentSessionRow, RunExit, RunExitReason, RunMode, SessionDetails, SessionOrigin, Store,
    StoreError, StoredActivity, new_id, now_ms,
};
use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::{
    CreateSpec, ExitReason as CoreExitReason, ExitRecord, ShellChoice, TerminalError,
    TerminalManager, TerminalState as CoreState,
};
use massorca_core_workspace::{DomainError, Workspaces};
use massorca_protocol::methods::agents::{
    AccountsChanged, Activity, ActivityState, AgentHookParams, AgentKinds, AgentModeSet,
    AgentOptionsSet, AgentSession, AgentSetOptionsParams, AgentStartParams, Execution,
    MAX_HOOK_PAYLOAD_BYTES, MAX_PREVIEW_CHARS, ModeApplied, ModeSource, OptionApplied,
    OptionsApplied, PromptReceipt, ReceiptKind, ResumeState, SessionFast, SessionMeta,
};
use massorca_protocol::methods::chat::RunType;
use massorca_protocol::methods::terminals::{
    DEFAULT_COLS, DEFAULT_ROWS, ExitReason, TerminalExit, TerminalState,
};
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceKind, WorkspaceState};
use massorca_protocol::{ErrorBody, codes};
use serde_json::{Value, json};

use crate::catalog::Catalog;
use crate::instructions::{self, DELEGATION_INSTRUCTIONS};
use crate::kind::{KindDecl, PasteMode, PromptMode, StatusMechanism};
use crate::launch::{self, LaunchError, LaunchInput, LaunchPlan, Run};
use crate::modes;
use crate::packages::KindPackages;
use crate::paths;
use crate::prompt;
use crate::provided_env::{
    EnvProvider, PROVIDER_TIMEOUT, ProvidedEnv, SecretGrant, provide_bounded,
};
use crate::resolve::{SearchEnv, VendorSearch, resolve_first};
use crate::resume_state::{self, ResumeInputs};
use crate::run_notes;
use crate::session_options::{self, ChosenOptions};
use crate::status::{self, HookRules};

mod archive;
mod chat;
mod delegation;
mod fork;
mod queue;

pub use chat::ChatOpened;
use chat::{ChatRun, RunStart, Started};
use delegation::DelegationView;
use fork::ForkFrom;

pub use delegation::{
    DELEGATION_DEPTH_DEFAULT, DELEGATION_DEPTH_MAX, DELEGATION_DEPTH_MIN, DELEGATION_DEPTH_SETTING,
    MergeResult, RepoOps,
};

/// How often the terminals of live runs are checked for an exit (0052).
pub const SWEEP_INTERVAL: Duration = Duration::from_millis(200);

/// How long a typed live command holds non-forced input and the queue, when no accepted hook report ends the hold first. A
/// slash command may send no report at all, so the hold must end by itself (decision 0171).
pub const TYPED_COMMAND_SETTLE: Duration = Duration::from_secs(2);

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
    /// A chat session's journal changed (decision 0101). Not a session change, so watchers do not get it; the chat streams
    /// of the session do.
    Chat(Box<crate::chat::ChatChanged>),
}

/// Receives every [`Change`]. Called under the status lock, so it must not block or call back into [`Agents`].
pub type Listener = Box<dyn Fn(Change) + Send + Sync>;

/// A prompt of the person that reached a session's run (decision 0118). It holds the text, so it is never printed.
pub struct PromptDelivered {
    /// The session the prompt reached.
    pub session: String,
    /// Its workspace.
    pub workspace: String,
    /// Its kind.
    pub kind: String,
    /// Whether it is a delegated child, which never names a workspace.
    pub child: bool,
    /// The prompt's text.
    pub text: String,
}

/// Prints every field but the text, which is replaced by its length, as a queued prompt is.
impl std::fmt::Debug for PromptDelivered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromptDelivered")
            .field("session", &self.session)
            .field("workspace", &self.workspace)
            .field("kind", &self.kind)
            .field("child", &self.child)
            .field("text", &format_args!("{} bytes", self.text.len()))
            .finish()
    }
}

/// Receives every prompt of the person once it is written. Called outside the status lock, so it must not block.
pub type PromptListener = Box<dyn Fn(PromptDelivered) + Send + Sync>;

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
            | LaunchError::ShimPromptTooLong
            | LaunchError::PromptHasNul
            | LaunchError::PromptHasControl => codes::AGENT_PROMPT_INVALID,
            LaunchError::NotResumable => codes::AGENT_RESUME_UNAVAILABLE,
            LaunchError::NotForkable => codes::AGENT_FORK_UNAVAILABLE,
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
    /// The search path agent programs are found on when kinds are listed and started: the host's, as the extension
    /// install check searches it (decision 0164).
    pub vendor_search: VendorSearch,
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
    prompt_listener: OnceLock<PromptListener>,
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
    /// Transcripts the resume state found (decision 0160), cleared when a run starts or exits.
    found_transcripts: resume_state::FoundTranscripts,
    /// The model listings the service keeps (decision 0166), held in memory.
    models: crate::model_catalog::ModelCache,
    /// The fast mode `agent.set_options` chose for each session, which its next resume launches with (decision 0171). The
    /// model and the effort are stored with the session (decision 0168); fast mode has no column, so it is held here, in
    /// memory: the service forgets it when it stops.
    chosen_fast: Mutex<HashMap<String, bool>>,
    /// The options each live run started with, updated by its live changes (decision 0171): a change is checked against them
    /// while the run is live. A stopped session is checked against what its next run keeps, which is stored instead.
    started: Mutex<HashMap<String, ChosenOptions>>,
    /// How long a model listing may run before it is stopped (decision 0166).
    listing_timeout: Duration,
    /// The chat runs whose end has not been recorded, by session (decision 0101). Taken after the status lock when both
    /// are held.
    chat_runs: Mutex<HashMap<String, Arc<ChatRun>>>,
    /// The journal writers of chat sessions, by session. Every journal change and every read that must line up with the
    /// stream (a snapshot or a replay) holds it, so a stream knows which batches its first event already covers. Taken
    /// after the status lock when both are held.
    journals: Mutex<HashMap<String, crate::chat::journal::JournalState>>,
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
    /// A typed live command holds non-forced input (prompts, live changes, the queue) until this instant, unless an accepted
    /// hook report ends the hold first. A slash command may send no report at all (decision 0171).
    settle_until: Option<Instant>,
    /// A mode switch is being typed into the run (decision 0083, R7): every other input waits for it, `force` or not, so
    /// two writes never interleave in the terminal.
    switching: bool,
    /// A queued prompt is being typed into the run (decision 0084): no other input waits for it, `force` or not.
    delivering: bool,
    /// The run reported `done` since the last prompt typed into it or reported (decision 0170): its turn is over, and a
    /// later `waiting` (an idle notification) does not reopen it. Set by a `done` report; cleared by a `working` report
    /// and by every prompt typed. A kind that never reports `done` never sets it.
    finished: bool,
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
    /// The model listings the service keeps (decision 0166).
    pub(crate) fn model_cache(&self) -> &crate::model_catalog::ModelCache {
        &self.inner.models
    }

    /// How long a model listing may run (decision 0166).
    pub(crate) fn listing_timeout(&self) -> Duration {
        self.inner.listing_timeout
    }
}

impl Agents {
    /// The service, with its exit sweep running every [`SWEEP_INTERVAL`] for as long as the service exists.
    pub fn new(config: AgentsConfig) -> Self {
        let agents = Self {
            inner: Arc::new(Inner {
                config,
                status: Mutex::new(HashMap::new()),
                listener: OnceLock::new(),
                prompt_listener: OnceLock::new(),
                start_gate: Mutex::new(()),
                accounts: Mutex::new(crate::accounts::AccountBusy::default()),
                bring_back_gate: Mutex::new(()),
                merging: Mutex::new(HashSet::new()),
                found_transcripts: resume_state::FoundTranscripts::default(),
                models: crate::model_catalog::ModelCache::default(),
                chosen_fast: Mutex::new(HashMap::new()),
                started: Mutex::new(HashMap::new()),
                listing_timeout: crate::model_catalog::listing_timeout_from(
                    std::env::var(crate::model_catalog::LISTING_TIMEOUT_ENV)
                        .ok()
                        .as_deref(),
                ),
                chat_runs: Mutex::new(HashMap::new()),
                journals: Mutex::new(HashMap::new()),
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

    /// Sets the receiver of every prompt of the person (decision 0118). Only the first call takes effect.
    pub fn set_prompt_listener(&self, listener: PromptListener) {
        let _ = self.inner.prompt_listener.set(listener);
    }

    /// Tells the prompt listener that the person's prompt reached `session`. A session that cannot be read is not told.
    pub(crate) fn announce_prompt(&self, session: &str, text: &str) {
        let Some(listener) = self.inner.prompt_listener.get() else {
            return;
        };
        let Ok(row) = self.reload(session) else {
            return;
        };
        listener(PromptDelivered {
            session: row.id,
            workspace: row.workspace_id,
            kind: row.kind,
            child: row.parent_id.is_some(),
            text: text.to_owned(),
        });
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

    /// The search path agent programs are found on: the host's, or the one the config names (decision 0164).
    pub(crate) fn program_search(&self) -> SearchEnv {
        self.inner.config.vendor_search.env()
    }

    /// Every declared kind, read again from the profile, with programs resolved on the program search (decision 0164).
    pub fn kinds(&self) -> AgentKinds {
        let profile_dir = self.profile_dir();
        let mut kinds = Catalog::load(self.kind_packages(), profile_dir)
            .views(profile_dir, &self.program_search());
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
        // A test service may start a fake kind as a chat run (decision 0101); a release build never does.
        #[cfg(feature = "test-hooks")]
        if crate::chat::is_test_chat_kind(&params.kind) {
            return self.start_chat(params, grant, Box::new(crate::chat::TestAdapter::default()));
        }
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
        self.start_session_as(params, parent, grant, RunStart::Terminal, None)
    }

    /// [`Self::start_session`], with the run in a terminal or as a chat (decision 0101). A chat's first prompt goes to
    /// its standard input once the session exists, never to its arguments.
    fn start_session_as(
        &self,
        params: AgentStartParams,
        parent: Option<&AgentSessionRow>,
        grant: SecretGrant,
        run: RunStart,
        fork: Option<&ForkFrom>,
    ) -> Result<AgentSession, AgentError> {
        let chat = matches!(run, RunStart::Chat(_));
        // A fork's conversation is a copy of its source's, so it holds what the source holds (decision 0168).
        let parent_holds = parent.is_some_and(|parent| parent.holds_secrets)
            || fork.is_some_and(|fork| fork.holds_secrets);
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
        let mode = modes::plan_start(
            &decl,
            params.permission_mode.as_deref(),
            Some(program.as_path()),
        )?
        .with_caller_args(&params.args);
        // The model and effort the kind's declared flags take (decision 0094), after the mode's own arguments.
        let mode_args = [
            mode.args.clone(),
            decl.option_args(params.model.as_deref(), params.effort.as_deref())?,
        ]
        .concat();
        // Fast mode, refused before anything starts when the kind or its model does not take it (decision 0165).
        let fast = decl.fast_for(params.model.as_deref(), params.fast.unwrap_or(false))?;
        if let Some(prompt) = &params.prompt {
            launch::check_prompt(prompt)?;
            // A kind that takes no prompt at launch gets it at its first idle, which only a status mechanism reports.
            if !chat && decl.prompt.mode == PromptMode::None && !decl.reports_idle() {
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
        // Only a worktree keeps one writing session at a time; a folder takes several (decision 0169). In a worktree, a
        // delegated child picked into its parent's worktree writes beside its delegation tree, which counts as one writer
        // there; the parent waits, and the tree's own refusals keep two of its sessions from writing at once (0170).
        let group = match parent {
            Some(parent) => Some(self.tree_root_of(parent)?),
            None => None,
        };
        if !params.read_only
            && keeps_one_writer(workspace.kind)
            && let Some(holder) = self.live_writer(&workspace.id, group.as_deref())?
        {
            let mut error = AgentError::new(
                codes::AGENT_WORKSPACE_BUSY,
                format!("session {holder} already writes in this workspace"),
            );
            error.data = Some(json!({ "session": holder }));
            return Err(error);
        }
        // The run is told how to delegate only when it may (decision 0161).
        let depth = match parent {
            Some(parent) => self.delegation_depth_of(parent)? + 1,
            None => 0,
        };
        let (instructions, instructions_withheld) =
            self.instructions_for(&decl, &program, depth)?;
        let inherited_path = std::env::var("PATH").ok();

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
            run: match fork {
                Some(fork) => Run::Fork {
                    prompt: params.prompt.as_deref(),
                    source_ref: &fork.source_ref,
                },
                None => Run::First {
                    prompt: params.prompt.as_deref().filter(|_| !chat),
                    new_ref: &new_ref,
                },
            },
            profile_dir: self.profile_dir(),
            massorca_bin: &self.inner.config.massorca_bin,
            credential_home: access.home.as_deref(),
            mode_args: &mode_args,
            caller_args: &params.args,
            provided_env: &provided,
            holds_secrets,
            caller_env: &params.env,
            instructions,
            inherited_path: inherited_path.as_deref(),
            fast,
        })?;

        let run_dir = paths::run_dir(self.profile_dir(), &session_id);
        // The start is recorded before its program starts and settled once the row exists, so a crash in between leaves
        // a mark the next service start finds (docs/architecture/agents.md, "Crash-safe start").
        mark_starting(&run_dir).inspect_err(|_| remove_dir(&run_dir))?;
        instructions::record_withheld(&run_dir, instructions_withheld.as_deref());
        run_notes::record(
            &run_dir,
            fork.map(|fork| fork.notes.as_slice()).unwrap_or_default(),
        );
        fast_mode::record(&run_dir, fast.is_some());
        // What the resume state reads later (decision 0160): the account this run uses, and no resume to classify.
        resume_state::record_probe(&run_dir, &decl, None)
            .and_then(|()| resume_state::record_run_account(&run_dir, access.account.as_deref()))
            .inspect_err(|_| remove_dir(&run_dir))?;
        self.inner.found_transcripts.clear();
        // The run is registered before its program starts, and the status lock is held until its row exists, so the
        // run's first hook report waits for both instead of being refused.
        let mut live = lock(&self.inner.status);
        // A chat run reports through its journal, not hooks, and takes prompts on standard input (decision 0101).
        if !chat {
            live.insert(
                session_id.clone(),
                LiveRun {
                    token,
                    rules: HookRules::of(&decl),
                    prompt: PromptRules::of(&decl),
                    pending: plan.pending_prompt.as_deref().map(PendingPrompt::new),
                    pending_switch: mode.switch_at_idle.as_deref().map(PendingPrompt::new),
                    typed_since_report: false,
                    settle_until: None,
                    switching: false,
                    delivering: false,
                    finished: false,
                    account: access.account.clone(),
                },
            );
        }
        let agent_ref = plan.agent_ref.clone();
        let size = PtySize {
            cols: params.cols.unwrap_or(DEFAULT_COLS),
            rows: params.rows.unwrap_or(DEFAULT_ROWS),
        };
        let title = params.title.clone().unwrap_or_else(|| decl.name.clone());
        let origin = SessionOrigin {
            model: params.model.clone(),
            effort: params.effort.clone(),
            forked_from: fork.map(|fork| fork.source_id.clone()),
        };
        let started = match run {
            RunStart::Terminal => self
                .spawn_run(plan, &workspace.id, &workspace.path, title, size)
                .map(Started::Terminal),
            RunStart::Chat(adapter) => self
                .spawn_chat_run(&session_id, plan, &workspace.id, &workspace.path, adapter)
                .map(Started::Chat),
        }
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
            terminal_id: match &started {
                Started::Terminal(terminal_id) => Some(terminal_id.clone()),
                Started::Chat(_) => None,
            },
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
        let chat_run = match &started {
            Started::Terminal(_) => None,
            Started::Chat(chat_run) => Some(chat_run.run_id.as_str()),
        };
        let inserted = self
            .store()
            .insert_agent_session_with_origin(&row, &origin, chat_run);
        if let Err(error) = inserted {
            live.remove(&session_id);
            // Closed once the status lock is released: a close can wait for the terminal's kill grace.
            drop(live);
            match &started {
                Started::Terminal(terminal_id) => {
                    let _ = self.inner.config.terminals.close(terminal_id);
                }
                Started::Chat(chat_run) => self.abandon_chat_run(&session_id, chat_run),
            }
            remove_dir(&run_dir);
            return Err(error.into());
        }
        clear_starting(&run_dir);
        // The session row now owns its conversation folder.
        if let Some(made) = made.as_mut() {
            made.keep();
        }
        let viewed = self.view_with(&mut live, row);
        if let Ok(session) = &viewed {
            self.notify(Change::Updated(Box::new(session.clone())));
        }
        drop(live);
        // Later starts need neither gate: a chat's first message is handed on outside them.
        drop((_gate, gate));
        let begun = match (&started, &viewed) {
            // Its journal opens, its output is read and its first prompt is sent only now that its row exists.
            (Started::Chat(chat_run), Ok(_)) => {
                self.begin_chat(&session_id, chat_run, params.prompt.as_deref())
            }
            _ => Ok(()),
        };
        let mut session = match viewed.and_then(|session| begun.map(|()| session)) {
            Ok(session) => session,
            Err(mut error) => {
                // A chat run whose start failed after its row exists is ended, so no program outlives the call; the
                // error names the session it leaves, ended.
                if let Started::Chat(chat_run) = &started {
                    self.abandon_chat_run(&session_id, chat_run);
                    error
                        .data
                        .get_or_insert_with(|| json!({ "session": session_id }));
                }
                return Err(error);
            }
        };
        session.secrets_withheld = withheld(grant);
        // Noted only once the start has succeeded, so a start that failed leaves no options behind (decision 0171).
        self.note_started(
            &session.id,
            ChosenOptions {
                model: params.model.clone(),
                effort: params.effort.clone(),
                fast: Some(fast.is_some()),
            },
        );
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
        let row = self.resolve(selector)?;
        // A chat session resumes as a chat (decision 0101, D9), never as a terminal run. Its kind's version probe can take
        // seconds, so it runs before the start gate is taken; a chat resume needs the same support as a chat start.
        let chat = self.run_type_of(&row.id)? == RunType::Chat;
        let mut adapter = if chat {
            let (decl, program) = self.resumable_kind(&row)?;
            Some(self.chat_adapter_of(&decl, &program)?)
        } else {
            None
        };
        let _gate = lock(&self.inner.start_gate);
        let row = self.resolve(&row.id)?;
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
            found: Some(&self.inner.found_transcripts),
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
            Some(program.as_path()),
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
        // A resumed child never writes beside another writer of its tree in a shared worktree, as a new child would not
        // (decision 0170); a folder takes several writers (decision 0169).
        if !row.read_only
            && keeps_one_writer(workspace.kind)
            && let Some(parent) = &row.parent_id
            && let Some(parent) = self.store().get_agent_session(parent)?
        {
            let holder = self.tree_writer(&lock(&self.inner.status), &parent, &workspace.id)?;
            if let Some(holder) = holder {
                let mut error = AgentError::new(
                    codes::AGENT_WORKSPACE_BUSY,
                    format!("session {holder} already writes in this workspace"),
                );
                error.data = Some(json!({ "session": holder }));
                return Err(error);
            }
        }
        // Only a worktree keeps one writing session at a time, where the session's delegation tree counts as one writer
        // (decisions 0169, 0170).
        let group = Some(self.tree_root_of(&row)?);
        if !row.read_only
            && keeps_one_writer(workspace.kind)
            && let Some(holder) = self.live_writer(&workspace.id, group.as_deref())?
        {
            let mut error = AgentError::new(
                codes::AGENT_WORKSPACE_BUSY,
                format!("session {holder} already writes in this workspace"),
            );
            error.data = Some(json!({ "session": holder }));
            return Err(error);
        }

        let depth = self.delegation_depth_of(&row)?;
        let (instructions, instructions_withheld) =
            self.instructions_for(&decl, &program, depth)?;
        let inherited_path = std::env::var("PATH").ok();
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
        // What the session records is kept (decision 0168): the model and the effort it names, where the kind still takes
        // them. The fast mode is the one `agent.set_options` chose for it (decision 0171).
        let origin = self
            .store()
            .agent_session_origin(&row.id)?
            .unwrap_or_default();
        let kept = kept_options(&decl, &origin);
        let chosen_fast = lock(&self.inner.chosen_fast)
            .get(&row.id)
            .copied()
            .unwrap_or(false);
        let mode_args = [mode.args.clone(), kept.args.clone()].concat();
        let fast = decl.fast_for(kept.model.as_deref(), chosen_fast)?;
        // Besides those options, a resumed run takes the kind's resume arguments only: the caller's arguments and
        // environment of the first start are not kept with the session (docs/architecture/agents.md).
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
            mode_args: &mode_args,
            caller_args: &[],
            provided_env: &provided,
            holds_secrets,
            caller_env: &no_env,
            instructions,
            inherited_path: inherited_path.as_deref(),
            // A resume takes the fast mode `agent.set_options` chose for the session, if any (decisions 0165, 0171).
            fast,
        })?;
        // The record names what the run uses from now on (decision 0168); a plan that fails leaves it as it was.
        if kept.model != origin.model || kept.effort != origin.effort {
            self.store().set_agent_session_options(
                &row.id,
                kept.model.as_deref(),
                kept.effort.as_deref(),
            )?;
        }
        let run_dir = paths::run_dir(self.profile_dir(), &row.id);
        // As in `start`: recorded before the program starts, settled once the row names the new terminal.
        mark_starting(&run_dir).inspect_err(|_| remove_dir(&run_dir))?;
        instructions::record_withheld(&run_dir, instructions_withheld.as_deref());
        run_notes::record(&run_dir, &kept.notes);
        fast_mode::record(&run_dir, fast.is_some());
        // The account this run uses, and the ref whose conversation its exit may show missing (decision 0160). The run
        // directory holds the earlier run's files, so it is kept on a failure here.
        resume_state::record_probe(&run_dir, &decl, Some(&agent_ref))?;
        resume_state::record_run_account(&run_dir, access.account.as_deref())?;
        self.inner.found_transcripts.clear();
        // As in `start`: the run is registered under the status lock before its program starts, and the lock is held until
        // the row names the new terminal, so the run's first report waits instead of being refused.
        let mut live = lock(&self.inner.status);
        // A chat run reports through its journal, not hooks (decision 0101).
        if !chat {
            live.insert(
                row.id.clone(),
                LiveRun {
                    token,
                    rules: HookRules::of(&decl),
                    prompt: PromptRules::of(&decl),
                    pending: None,
                    pending_switch: mode.switch_at_idle.as_deref().map(PendingPrompt::new),
                    typed_since_report: false,
                    settle_until: None,
                    switching: false,
                    delivering: false,
                    finished: false,
                    account: access.account.clone(),
                },
            );
        }
        let title = row.title.clone().unwrap_or_else(|| decl.name.clone());
        let size = PtySize {
            cols: cols.unwrap_or(DEFAULT_COLS),
            rows: rows.unwrap_or(DEFAULT_ROWS),
        };
        let started = match adapter.take() {
            None => self
                .spawn_run(plan, &workspace.id, &workspace.path, title, size)
                .map(Started::Terminal),
            Some(adapter) => self
                .spawn_chat_run(&row.id, plan, &workspace.id, &workspace.path, adapter)
                .map(Started::Chat),
        }
        .inspect_err(|_| {
            live.remove(&row.id);
            remove_dir(&run_dir);
        })?;
        crash_point("agent-resume-spawned");
        let restarted = match &started {
            Started::Terminal(terminal_id) => self.store().restart_agent_session(
                &row.id,
                terminal_id,
                holds_secrets,
                &mode.mode,
                still_pending.as_deref(),
            ),
            Started::Chat(chat_run) => self.store().restart_chat_agent_session(
                &row.id,
                &chat_run.run_id,
                holds_secrets,
                &mode.mode,
                still_pending.as_deref(),
            ),
        };
        if !matches!(restarted, Ok(true)) {
            live.remove(&row.id);
            // Closed once the status lock is released: a close can wait for the terminal's kill grace.
            drop(live);
            match &started {
                Started::Terminal(terminal_id) => {
                    let _ = self.inner.config.terminals.close(terminal_id);
                }
                Started::Chat(chat_run) => self.abandon_chat_run(&row.id, chat_run),
            }
            remove_dir(&run_dir);
            restarted?;
            return Err(AgentError::new(
                codes::AGENT_NOT_FOUND,
                format!("no agent session {}", row.id),
            ));
        }
        clear_starting(&run_dir);
        // A session run again is in use again: its archive is cleared, so lists show it (decision 0163).
        self.store().set_agent_session_archived(&row.id, None)?;
        let fresh = self.reload(&row.id)?;
        if let Started::Terminal(terminal_id) = &started {
            if let Some(previous) = row.terminal_id.as_deref().filter(|old| old != terminal_id) {
                // The previous run's terminal is no longer the session's: unpinned, it is evicted like any exited
                // terminal.
                let _ = self.inner.config.terminals.pin(previous, false);
            }
            if fresh.retained {
                // A retained session keeps every run's terminal from eviction, not only the first (decision 0090).
                let _ = self.inner.config.terminals.pin(terminal_id, true);
            }
        }
        let mut session = self.view_with(&mut live, fresh)?;
        self.notify(Change::Updated(Box::new(session.clone())));
        drop(live);
        if let Started::Chat(chat_run) = &started {
            // The journal carries on in its epoch; the run's output is let in now that the row names it (decision
            // 0101, D9). A resume sends no prompt.
            drop((_gate, gate));
            if let Err(mut error) = self.begin_chat(&row.id, chat_run, None) {
                self.abandon_chat_run(&row.id, chat_run);
                error
                    .data
                    .get_or_insert_with(|| json!({ "session": row.id }));
                return Err(error);
            }
        }
        session.secrets_withheld = withheld(grant);
        // Noted only once the resume has succeeded (decision 0171).
        self.note_started(
            &session.id,
            ChosenOptions {
                model: kept.model.clone(),
                effort: kept.effort.clone(),
                fast: Some(fast.is_some()),
            },
        );
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
        let program = resolve_first(&decl.program, &self.program_search()).ok_or_else(|| {
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
        let learned = self.kept_learned_ref(&row, learned)?;
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
        // A `done` finishes the turn and a `working` opens one; only the first `done` of a turn is new (decision 0170).
        let mut newly_finished = false;
        if let Some(run) = live.get_mut(&params.session) {
            run.typed_since_report = false;
            // Any accepted report ends a settle window early (decision 0171).
            run.settle_until = None;
            match row
                .activity
                .as_ref()
                .map(|activity| activity.state.as_str())
            {
                Some("done") => {
                    newly_finished = !run.finished;
                    run.finished = true;
                }
                Some("working") => run.finished = false,
                _ => {}
            }
        }
        // A delegated child's ended turn is told to its parent, through the parent's queue, once (decision 0170).
        let parent = match row.activity.as_ref() {
            Some(activity) if newly_finished && row.parent_id.is_some() => {
                let outcome = delegation::turn_outcome(activity.outcome.as_deref());
                let key = format!("done:{}", activity.at);
                self.queue_child_notice(&live, &row, &outcome, &key)
            }
            _ => None,
        };
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
                    run.finished = false;
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
        // The notice, and any prompt the child's work held (decision 0170), may go now.
        if let Some(parent) = parent {
            self.drain_queue(&parent);
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
        let receipt = self.type_prompt(&row, text, force, lift_queue_pauses)?;
        // Only the person's prompts are announced: a system prompt, such as a delegated child's report, is not work.
        if lift_queue_pauses {
            self.announce_prompt(&row.id, text);
        }
        Ok(receipt)
    }

    /// Types `text` into the session `row` names, a terminal or a chat, as [`Agents::send_prompt`] does.
    fn type_prompt(
        &self,
        row: &AgentSessionRow,
        text: &str,
        force: bool,
        lift_queue_pauses: bool,
    ) -> Result<PromptReceipt, AgentError> {
        // A chat session takes a stream-json message instead of typing (decision 0101, D7). A chat parent waits for its
        // writing child as a terminal one does (decision 0170).
        if self.run_type_of(&row.id)? == RunType::Chat {
            if !force && let Some(child) = self.writing_child(&lock(&self.inner.status), row) {
                return Err(child_writing(&row.id, &child));
            }
            return self.chat_prompt(row, text, force, lift_queue_pauses);
        }
        // The readiness check and the copy of the input rules happen under the status lock. The write does not: it can
        // wait for the agent to read, and the agent's own hook reports need the lock to return.
        let (session_id, terminal, bytes) = {
            let mut live = lock(&self.inner.status);
            let row = self.reload(&row.id)?;
            let session = self.view_with(&mut live, row)?;
            // A bring-back merging into the workspace holds it until the merge ends (0055); `force` does not override
            // this, because a prompt could make the agent write into the worktree while git merges into it.
            self.refuse_while_merging(&session.workspace_id)?;
            // A parent waits while its writing child works in its workspace (decision 0170); force types anyway.
            if !force && let Some(child) = self.writing_child(&live, &self.reload(&session.id)?) {
                return Err(child_writing(&session.id, &child));
            }
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
            run.finished = false;
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

    /// Changes a session's model, effort or fast mode (decision 0171). A model or fast mode the kind declares a live command
    /// for is typed into a live run once the agent is ready; every other change, and every change to a session whose run is
    /// not live, waits for the session's next resume. The values are checked before anything is typed or stored.
    pub fn set_options(
        &self,
        request: &AgentSetOptionsParams,
    ) -> Result<AgentOptionsSet, AgentError> {
        let row = self.resolve(&request.session)?;
        let decl = self.declared_kind(&row.kind)?;
        let run_dir = paths::run_dir(self.profile_dir(), &row.id);
        // Planned under the status lock, where the run's liveness and recorded fast mode are read, so two calls never plan
        // the same toggle. The text is typed outside the lock, as a prompt is, because the write can wait for the agent.
        let mut live = lock(&self.inner.status);
        let row = self.reload(&row.id)?;
        let session = self.view_with(&mut live, row)?;
        let running = live.contains_key(&session.id) && session.terminal_id.is_some();
        let recorded_fast = fast_mode::of_run(&run_dir).is_some();
        let chosen = self.chosen_of(&session.id)?;
        let started = self.started_of(&session.id);
        let plan = session_options::plan(
            &decl,
            &session.id,
            request,
            &started,
            &chosen,
            running,
            recorded_fast,
        )?;
        let typing = if plan.typed.is_empty() {
            None
        } else {
            let terminal = session.terminal_id.clone().unwrap_or_default();
            let Some(run) = live.get_mut(&session.id) else {
                return Err(not_live(&session.id));
            };
            let state = session.activity.as_ref().map(|activity| activity.state);
            if let Some(refusal) = input_refusal(run, &session.id, state, false) {
                return Err(refusal);
            }
            run.switching = true;
            let bytes: Vec<Vec<u8>> = plan
                .typed
                .iter()
                .map(|text| prompt::input_bytes(text, PasteMode::Plain, &run.prompt.submit))
                .collect();
            Some((terminal, bytes))
        };
        drop(live);
        if let Some((terminal, bytes)) = typing {
            // The commands are typed in turn. The run stays marked as switching until the writes are done, whatever their
            // outcome, and the queue may go on after them (decision 0084, D3).
            let written: Result<(), TerminalError> = bytes
                .iter()
                .try_for_each(|bytes| self.inner.config.terminals.input(&terminal, bytes));
            if written.is_ok() {
                // Recorded before the run is free again, so the next call and the queue see what was typed.
                if let Some(on) = plan.fast_typed {
                    fast_mode::record(&run_dir, on);
                }
                self.note_typed(&session.id, &plan, request);
            }
            self.end_switching(&session.id);
            self.drain_queue(&session.id);
            if let Err(error) = written {
                return Err(match error {
                    TerminalError::Exited | TerminalError::NotFound => not_live(&session.id),
                    other => AgentError::from(other),
                });
            }
        }
        let changed = plan.applied != OptionsApplied::default();
        {
            // One read and one write under the lock, so what another call stored meanwhile survives (decision 0171): the
            // requested fields are written to the stored columns, and the fast mode is kept in memory.
            let mut chosen_fast = lock(&self.inner.chosen_fast);
            let origin = self
                .store()
                .agent_session_origin(&session.id)?
                .unwrap_or_default();
            let current = ChosenOptions {
                model: origin.model,
                effort: origin.effort,
                fast: chosen_fast.get(&session.id).copied(),
            };
            let merged = session_options::merged(current, request, &plan.chosen);
            self.store().set_agent_session_options(
                &session.id,
                merged.model.as_deref(),
                merged.effort.as_deref(),
            )?;
            match merged.fast {
                Some(on) => chosen_fast.insert(session.id.clone(), on),
                None => chosen_fast.remove(&session.id),
            };
        }
        let mut live = lock(&self.inner.status);
        let session = self.view_with(&mut live, self.reload(&session.id)?)?;
        if changed {
            self.notify(Change::Updated(Box::new(session.clone())));
        }
        Ok(AgentOptionsSet {
            applied: plan.applied,
            session,
        })
    }

    /// Records what a live change typed (decision 0171): its settle window, and the options the run now runs with.
    fn note_typed(&self, id: &str, plan: &session_options::Plan, request: &AgentSetOptionsParams) {
        self.settle_after_typed(id);
        let mut started = lock(&self.inner.started);
        let entry = started.entry(id.to_owned()).or_default();
        if plan.applied.model == Some(OptionApplied::Live) {
            entry.model = request.model.clone();
        }
        if let Some(on) = plan.fast_typed {
            entry.fast = Some(on);
        }
    }

    /// Starts the settle window of a typed live command (decision 0171): until it ends, non-forced prompts, live changes and
    /// the queue wait. A timer ends the window when no report does, and then drains the queue it held.
    fn settle_after_typed(&self, id: &str) {
        let until = Instant::now() + TYPED_COMMAND_SETTLE;
        if let Some(run) = lock(&self.inner.status).get_mut(id) {
            run.settle_until = Some(until);
        }
        let weak = Arc::downgrade(&self.inner);
        let id = id.to_owned();
        thread::spawn(move || {
            thread::sleep(until.saturating_duration_since(Instant::now()));
            if let Some(inner) = weak.upgrade() {
                Agents { inner }.end_settle(&id, until);
            }
        });
    }

    /// Ends the settle window `until`, if no report ended it first, and drains the queue it held (decision 0171).
    fn end_settle(&self, id: &str, until: Instant) {
        let ended = match lock(&self.inner.status).get_mut(id) {
            Some(run) if run.settle_until == Some(until) => {
                run.settle_until = None;
                true
            }
            _ => false,
        };
        if ended {
            self.drain_queue(id);
        }
    }

    /// The options session `id`'s live run started with, as its live changes have moved them (decision 0171).
    fn started_of(&self, id: &str) -> ChosenOptions {
        lock(&self.inner.started)
            .get(id)
            .cloned()
            .unwrap_or_default()
    }

    /// Records the options a run of session `id` started with (decision 0171).
    fn note_started(&self, id: &str, options: ChosenOptions) {
        lock(&self.inner.started).insert(id.to_owned(), options);
    }

    /// What session `id`'s next run keeps (decisions 0168, 0171): the model and effort of its stored columns, and the fast
    /// mode `agent.set_options` chose for it.
    fn chosen_of(&self, id: &str) -> Result<ChosenOptions, AgentError> {
        let origin = self.store().agent_session_origin(id)?.unwrap_or_default();
        Ok(ChosenOptions {
            model: origin.model,
            effort: origin.effort,
            fast: lock(&self.inner.chosen_fast).get(id).copied(),
        })
    }

    /// The first bypass mode (decision 0083) that kind `id` declares and a new run of it can apply: one with launch
    /// arguments, or a live switch on a kind that reports idle. `None` when it declares none, or none a start can apply.
    pub fn startable_bypass_mode(&self, id: &str) -> Option<String> {
        let decl = self.declared_kind(id).ok()?;
        decl.permission_modes
            .iter()
            .find(|mode| mode.bypass && modes::plan(&decl, mode, None).is_ok())
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
    /// A first input it drops is noted on its run and sent to watchers with the record (decision 0168).
    pub fn sweep(&self) {
        let mut live = lock(&self.inner.status);
        let dropped = expire_first_prompts(&mut live, Instant::now());
        for id in &dropped {
            tracing::warn!(
                session = %id,
                "the first input was dropped: the agent reported no idle state in time"
            );
            run_notes::append(
                &paths::run_dir(self.profile_dir(), id),
                &first_input_dropped_note(),
            );
        }
        let ids: Vec<String> = live.keys().cloned().collect();
        for id in ids {
            // Only a dropped input's record is read in full: the sweep runs every tick and reads the execution alone.
            let notify = dropped.contains(&id);
            let checked = match self.store().get_agent_session(&id) {
                Ok(Some(row)) => self.view_in(&mut live, row, None, notify).map(|session| {
                    if notify {
                        self.notify(Change::Updated(Box::new(session)));
                    }
                }),
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
        // One catalog for the whole list: each row's resume state reads its kind from it (decision 0160).
        let catalog = Catalog::load(self.kind_packages(), self.profile_dir());
        self.store()
            .list_agent_sessions(workspace_id.as_deref())?
            .into_iter()
            .map(|row| {
                let mut live = lock(&self.inner.status);
                let row = self.store().get_agent_session(&row.id)?.unwrap_or(row);
                self.view_in(&mut live, row, Some(&catalog), true)
            })
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
        if row.exit.is_none() && row.terminal_id.is_none() {
            self.stop_chat_run(&row.id, force);
        }
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
        lock(&self.inner.chosen_fast).remove(&session.id);
        lock(&self.inner.started).remove(&session.id);
        self.forget_chat(&session.id);
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
        // Pinned before the flag is saved, so a failed pin leaves the session as it was. An archived session keeps its
        // terminal pinned whatever its retain says (decision 0163).
        let archived = self.store().agent_session_archived_at(&id)?.is_some();
        if let Some(terminal) = &self.reload(&id)?.terminal_id {
            match self
                .inner
                .config
                .terminals
                .pin(terminal, retained || archived)
            {
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
        let program = resolve_first(&decl.program, &self.program_search()).ok_or_else(|| {
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
    /// The instructions a run of `decl` at `depth` gets, and the note its session records when its CLI fails the
    /// channel's support probe (decision 0161). None while the session may not delegate, or the kind has no channel.
    fn instructions_for(
        &self,
        decl: &KindDecl,
        program: &Path,
        depth: i64,
    ) -> Result<(Option<&'static str>, Option<String>), AgentError> {
        if decl.instructions.is_none() || !self.may_delegate_at(depth)? {
            return Ok((None, None));
        }
        Ok(if instructions::channel_supported(decl, program) {
            (Some(DELEGATION_INSTRUCTIONS), None)
        } else {
            (None, Some(instructions::withheld_note(decl)))
        })
    }

    /// A session whose run writes in the workspace now, outside the delegation tree rooted at `group` when one is given
    /// (decision 0170).
    fn live_writer(
        &self,
        workspace_id: &str,
        group: Option<&str>,
    ) -> Result<Option<String>, AgentError> {
        for row in self.store().list_agent_sessions(Some(workspace_id))? {
            if row.read_only {
                continue;
            }
            if let Some(root) = group
                && self.tree_root_of(&row)? == root
            {
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
    /// The fast mode of a session's current or last run (decision 0165).
    fn fast_of(&self, session: &str) -> Option<SessionFast> {
        fast_mode::of_run(&paths::run_dir(self.profile_dir(), session))
    }

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
        row: AgentSessionRow,
    ) -> Result<AgentSession, AgentError> {
        self.view_in(live, row, None, true)
    }

    /// [`Self::view_with`], reading the kind from `catalog` when the caller loaded one. `full` also reads what a caller
    /// shows beside the record: the start options, lineage, branch and preview, and the notes of the run (decision 0168).
    /// The exit sweep passes `false` on every tick and reads the execution alone.
    fn view_in(
        &self,
        live: &mut LiveRuns,
        mut row: AgentSessionRow,
        catalog: Option<&Catalog>,
        full: bool,
    ) -> Result<AgentSession, AgentError> {
        let run = self.run_of(&row)?;
        let execution = self.execution_in(live, &mut row, run)?;
        let resume = self.resume_of(&row, execution.state == TerminalState::Exited, catalog);
        let placement = self.delegation_view(&row);
        // The archived flag and the folder writers are read for the views a caller sees; the sweep reads the execution alone.
        let archived = full && self.store().agent_session_archived_at(&row.id)?.is_some();
        let fast = self.fast_of(&row.id);
        let shared_folder_with = if !full || execution.state == TerminalState::Exited {
            Vec::new()
        } else {
            self.shared_folder_with(live, &row)?
        };
        let mut session = record(row, execution, resume, placement, run, archived, fast);
        session.shared_folder_with = shared_folder_with;
        if full {
            let run_dir = paths::run_dir(self.profile_dir(), &session.id);
            session.notes.extend(run_notes::read(&run_dir));
            let details = self.store().agent_session_details(&session.id)?;
            with_details(&mut session, details.unwrap_or_default());
        }
        Ok(session)
    }

    /// The run a row is: a terminal run, or a chat. Only a session without a terminal can be a chat; the store says
    /// which (decision 0101).
    fn run_of(&self, row: &AgentSessionRow) -> Result<RunType, AgentError> {
        match &row.terminal_id {
            Some(_) => Ok(RunType::Terminal),
            None => self.run_type_of(&row.id),
        }
    }

    /// The execution of a row's run, read now. An end the terminal reports is applied to `row` as in [`Self::view_with`].
    /// It never reads the session's own view, so it can be called for a sibling session without recursion.
    fn execution_in(
        &self,
        live: &mut LiveRuns,
        row: &mut AgentSessionRow,
        run: RunType,
    ) -> Result<Execution, AgentError> {
        let execution = match (&row.exit, &row.terminal_id) {
            (Some(exit), _) => {
                live.remove(&row.id);
                exited(exit)
            }
            (None, None) if run == RunType::Chat => self.chat_execution(live, row)?,
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
                            self.record_exit(live, row, &terminal, exit)?
                        }
                    },
                    Err(TerminalError::NotFound) => {
                        self.record_exit(live, row, &terminal, gone())?
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        };
        Ok(execution)
    }

    /// The other live writing sessions of the folder workspace a live writing session `row` writes in, in the store's
    /// order (decision 0169). Empty for a read-only session and for a workspace that is not a folder, since a worktree
    /// keeps one writer.
    fn shared_folder_with(
        &self,
        live: &mut LiveRuns,
        row: &AgentSessionRow,
    ) -> Result<Vec<String>, AgentError> {
        if row.read_only {
            return Ok(Vec::new());
        }
        let mut others = Vec::new();
        for mut other in self.store().list_agent_sessions(Some(&row.workspace_id))? {
            if other.id == row.id || other.read_only {
                continue;
            }
            let run = self.run_of(&other)?;
            if self.execution_in(live, &mut other, run)?.state != TerminalState::Exited {
                others.push(other.id);
            }
        }
        // The kind is read only when another writer exists, so a session alone in its workspace costs no more.
        if others.is_empty() {
            return Ok(others);
        }
        let folder = self
            .store()
            .get_workspace(&row.workspace_id)?
            .is_some_and(|workspace| workspace.kind == massorca_core_store::WorkspaceKind::Folder);
        Ok(if folder { others } else { Vec::new() })
    }

    /// The learned ref of a run, unless it is the ref of the session's fork source: a fork that kept its source's ref would
    /// resume the source's transcript, so the ref is dropped and the fork keeps none (decision 0168).
    fn kept_learned_ref(
        &self,
        row: &AgentSessionRow,
        learned: Option<String>,
    ) -> Result<Option<String>, AgentError> {
        let Some(learned) = learned else {
            return Ok(None);
        };
        let Some(source) = self
            .store()
            .agent_session_origin(&row.id)?
            .and_then(|origin| origin.forked_from)
        else {
            return Ok(Some(learned));
        };
        let source_ref = self
            .store()
            .get_agent_session(&source)?
            .and_then(|source| source.agent_ref);
        if source_ref.as_deref() == Some(learned.as_str()) {
            tracing::warn!(
                session = %row.id,
                "a fork reported its source's agent ref, which is not kept"
            );
            return Ok(None);
        }
        Ok(Some(learned))
    }

    /// The session's resume state (decision 0160), from its kind as declared now: in `catalog`, else a catalog loaded
    /// for this view. The transcript is looked for only once the run has exited, and a found one is kept in memory
    /// until a run starts or exits.
    fn resume_of(
        &self,
        row: &AgentSessionRow,
        exited: bool,
        catalog: Option<&Catalog>,
    ) -> ResumeState {
        let loaded;
        let catalog = match catalog {
            Some(catalog) => catalog,
            None => {
                loaded = Catalog::load(self.kind_packages(), self.profile_dir());
                &loaded
            }
        };
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
            found: Some(&self.inner.found_transcripts),
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
        self.record_run_exit(live, row, RunRef::Terminal(terminal), exit)
    }

    /// [`Self::record_exit`] for a run in a terminal or a chat run (decision 0101): the same rules, with the end recorded
    /// only while the row still names that run.
    fn record_run_exit(
        &self,
        live: &mut LiveRuns,
        row: &mut AgentSessionRow,
        run: RunRef<'_>,
        exit: RunExit,
    ) -> Result<Execution, AgentError> {
        let changed = match run {
            RunRef::Terminal(terminal) => self
                .store()
                .set_agent_session_exit(&row.id, terminal, &exit)?,
            RunRef::Chat(run_id) => self
                .store()
                .set_agent_session_chat_exit(&row.id, run_id, &exit)?,
        };
        let catalog = changed.then(|| Catalog::load(self.kind_packages(), self.profile_dir()));
        // A turn that ended before the exit was told to the parent then (decision 0170).
        let told = live.get(&row.id).is_some_and(|run| run.finished);
        if let Some(catalog) = &catalog {
            live.remove(&row.id);
            self.inner.found_transcripts.clear();
            // The exit rule reads the activity and result as they are now, under the status lock.
            if let Some(fresh) = self.store().get_agent_session(&row.id)? {
                *row = fresh;
            }
            // A resumed run that failed early because its CLI found no conversation leaves the session unable to
            // resume (decision 0160). A resume clears the activity, so one here means the run reported.
            let facts = resume_state::ExitFacts {
                failed: exit.code.is_some_and(|code| code != 0),
                reported: row.activity.is_some(),
            };
            resume_state::classify_exit(
                &paths::run_dir(self.profile_dir(), &row.id),
                catalog.find(&row.kind).and_then(|entry| entry.valid()),
                facts,
                || match run {
                    RunRef::Terminal(terminal) => self.inner.config.terminals.text(terminal).ok(),
                    RunRef::Chat(_) => self.chat_output(&row.id),
                },
            );
            let cut = status::on_exit(row.activity.as_ref(), exit.at);
            if let Some(activity) = &cut {
                self.store().set_agent_session_status(
                    &row.id,
                    Some(activity),
                    row.result.as_deref(),
                    None,
                )?;
                row.activity = cut.clone();
            }
            if !told && row.parent_id.is_some() {
                let outcome = delegation::exit_outcome(&exit, cut.is_some());
                let key = format!("exit:{}", exit.at);
                // The caller holds the status lock, so the parent's queue is drained on a thread of its own.
                if let Some(parent) = self.queue_child_notice(live, row, &outcome, &key) {
                    self.drain_parent_later(parent);
                }
            }
        }
        let execution = exited(&exit);
        row.exit = Some(exit);
        if let Some(catalog) = &catalog {
            let resume = self.resume_of(row, true, Some(catalog));
            let placement = self.delegation_view(row);
            let run_type = match run {
                RunRef::Terminal(_) => RunType::Terminal,
                RunRef::Chat(_) => RunType::Chat,
            };
            let archived = self.store().agent_session_archived_at(&row.id)?.is_some();
            let fast = self.fast_of(&row.id);
            self.notify(Change::Updated(Box::new(record(
                row.clone(),
                execution.clone(),
                resume,
                placement,
                run_type,
                archived,
                fast,
            ))));
        }
        Ok(execution)
    }
}

/// The run an end belongs to: the terminal of a terminal run, or the id of a chat run (decision 0101).
#[derive(Debug, Clone, Copy)]
enum RunRef<'a> {
    Terminal(&'a str),
    Chat(&'a str),
}

/// The wire record of a row with its execution, its resume state, what it may delegate, its run type, whether it is
/// archived and its fast mode (decision 0165).
fn record(
    row: AgentSessionRow,
    execution: Execution,
    resume: ResumeState,
    placement: DelegationView,
    run: RunType,
    archived: bool,
    fast: Option<SessionFast>,
) -> AgentSession {
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
        run,
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
        archived,
        mode_source: row.mode.source.as_deref().map(mode_source_of),
        pending_permission_mode: row.pending_permission_mode,
        resume: Some(resume),
        delegation: placement.options,
        notes: placement.notes,
        fast,
        shared_folder_with: Vec::new(),
        meta: Default::default(),
    }
}

/// Whether a workspace of `kind` takes one writing session at a time (`agent.workspace_busy`). Only a worktree does: a
/// folder has no git index to corrupt, so several sessions may write in it (decision 0169). A kind a newer peer adds
/// is not a folder, so it keeps the refusal.
fn keeps_one_writer(kind: WorkspaceKind) -> bool {
    match kind {
        WorkspaceKind::Worktree | WorkspaceKind::Unknown => true,
        WorkspaceKind::Folder => false,
    }
}

/// What a resume or a fork passes again of the model and the effort `origin` names (decision 0168): the ones the kind
/// still takes, and a note for each one it no longer takes, which the run records. A model is kept when
/// [`KindDecl::takes_model`] accepts it; an effort when the kind's effort flag accepts it for the kept model.
#[derive(Debug, Default, PartialEq, Eq)]
struct KeptOptions {
    model: Option<String>,
    effort: Option<String>,
    /// The launch arguments of the kept model and effort, the model's first.
    args: Vec<String>,
    notes: Vec<String>,
}

fn kept_options(decl: &KindDecl, origin: &SessionOrigin) -> KeptOptions {
    let mut kept = KeptOptions::default();
    if let Some(model) = origin.model.as_deref() {
        if decl.takes_model(model) {
            kept.model = Some(model.to_owned());
        } else {
            kept.notes.push(format!(
                "The model {model} is no longer accepted by {}, so this run did not use it.",
                decl.name
            ));
        }
    }
    if let Some(effort) = origin.effort.as_deref() {
        if decl
            .option_args(kept.model.as_deref(), Some(effort))
            .is_ok()
        {
            kept.effort = Some(effort.to_owned());
        } else {
            kept.notes.push(format!(
                "The effort {effort} is no longer accepted by {}, so this run did not use it.",
                decl.name
            ));
        }
    }
    // Each kept option passed its own check above, so the arguments of both build.
    kept.args = decl
        .option_args(kept.model.as_deref(), kept.effort.as_deref())
        .unwrap_or_default();
    kept
}

/// The note a run records once its first input was dropped (decision 0168).
fn first_input_dropped_note() -> String {
    format!(
        "The first input was dropped: the agent reported no idle state within {} seconds.",
        FIRST_PROMPT_WAIT.as_secs()
    )
}

/// Sets what a session view shows beside its row (decision 0168): its start options, its lineage, its workspace's
/// branch, and a preview, which is a chat's newest message or a terminal session's result.
fn with_details(session: &mut AgentSession, details: SessionDetails) {
    let source = match session.run {
        RunType::Chat => details.last_message,
        _ => session.result.clone(),
    };
    session.meta = SessionMeta {
        model: details.origin.model,
        effort: details.origin.effort,
        branch: details.branch,
        preview: source.as_deref().and_then(preview_of),
        forked_from: details.origin.forked_from,
    };
}

/// At most [`MAX_PREVIEW_CHARS`] characters of `text`, its line breaks and runs of white space made single spaces;
/// `None` when nothing is left.
fn preview_of(text: &str) -> Option<String> {
    let mut preview = String::new();
    for word in text.split_whitespace() {
        if !preview.is_empty() {
            preview.push(' ');
        }
        preview.push_str(word);
        if preview.chars().count() >= MAX_PREVIEW_CHARS {
            break;
        }
    }
    let preview: String = preview.chars().take(MAX_PREVIEW_CHARS).collect();
    (!preview.is_empty()).then_some(preview)
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
    // A typed live command holds non-forced input for its settle window (decision 0171). `force` and `send_now` pass.
    if !force && run.settle_until.is_some_and(|until| Instant::now() < until) {
        return Some(AgentError::new(
            codes::AGENT_BUSY,
            format!(
                "session {session} is settling after a typed command: try again in a moment, or use force"
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

/// The refusal of a prompt to a parent whose writing child works in its workspace (decision 0170), naming the child in
/// `data.child`.
fn child_writing(session: &str, child: &str) -> AgentError {
    let mut error = AgentError::new(
        codes::AGENT_BUSY,
        format!(
            "session {session} waits while its child session {child} writes in the same workspace; queue the prompt, or use force"
        ),
    );
    error.data = Some(json!({ "child": child }));
    error
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
pub(crate) fn resume_unavailable(session: &str, reason: &str) -> AgentError {
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

/// The period of the background sweep. Unit tests call [`Agents::sweep`] themselves, so their background sweep waits a
/// minute: a test that runs longer than one tick on a loaded machine would otherwise lose a live run it is still using.
fn sweep_interval() -> Duration {
    if cfg!(test) {
        Duration::from_secs(60)
    } else {
        SWEEP_INTERVAL
    }
}

/// Sweeps every [`SWEEP_INTERVAL`] (see [`sweep_interval`]) until the service is gone.
fn sweep_loop(service: &Weak<Inner>) {
    loop {
        thread::sleep(sweep_interval());
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

    /// [`service`], with kinds from `packages`, and the process `PATH` as the program search.
    pub(crate) fn service_with(
        packages: Arc<dyn KindPackages>,
    ) -> (Agents, String, tempfile::TempDir) {
        service_searching(packages, VendorSearch::fixed(SearchEnv::current()))
    }

    /// [`service_with`], with the programs searched for on `vendor_search`.
    pub(crate) fn service_searching(
        packages: Arc<dyn KindPackages>,
        vendor_search: VendorSearch,
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
            name_generated: false,
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
            vendor_search,
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
            settle_until: None,
            switching,
            delivering: false,
            finished: false,
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
        // A typed live command settles the run for its window (decision 0171): unforced input waits, forced input goes
        // through, and a window that has ended refuses nothing.
        let mut settling = run(false);
        settling.settle_until = Some(Instant::now() + Duration::from_secs(3600));
        assert!(input_refusal(&settling, "s", idle, false).is_some());
        assert!(
            input_refusal(&settling, "s", idle, true).is_none(),
            "force goes through"
        );
        settling.settle_until = Some(Instant::now() - Duration::from_millis(1));
        assert!(
            input_refusal(&settling, "s", idle, false).is_none(),
            "an ended window refuses nothing"
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
            settle_until: None,
            switching: false,
            delivering: false,
            finished: false,
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

    /// A first input the exit sweep drops is noted on its run, where a session view reads it (decision 0168).
    #[test]
    fn a_dropped_first_input_is_noted_on_its_run() {
        let (agents, _workspace_id, profile) = service();
        lock(&agents.inner.status).insert(
            "late".to_owned(),
            LiveRun {
                token: "t".to_owned(),
                rules: HookRules::default(),
                prompt: PromptRules {
                    has_status: true,
                    paste: PasteMode::Plain,
                    submit: "\r".to_owned(),
                },
                pending: Some(PendingPrompt {
                    text: "later".to_owned(),
                    due: Instant::now(),
                }),
                pending_switch: None,
                typed_since_report: false,
                settle_until: None,
                switching: false,
                delivering: false,
                finished: false,
                account: None,
            },
        );
        // A live run's directory exists: the start made it.
        std::fs::create_dir_all(paths::run_dir(profile.path(), "late")).unwrap();
        agents.sweep();
        assert_eq!(
            run_notes::read(&paths::run_dir(profile.path(), "late")),
            [first_input_dropped_note()]
        );
    }

    /// A resume or a fork passes again the stored model and effort the kind still takes, and notes each one it does not
    /// (decision 0168). A model is left out when the kind's choices no longer list it; an effort when the effort levels,
    /// or the kept model's own efforts, no longer list it.
    #[test]
    fn a_resume_or_fork_keeps_the_options_the_kind_still_takes_and_notes_the_rest() {
        let decl = KindDecl::from_json(
            &json!({
                "id": "fake",
                "name": "Fake",
                "program": ["fake"],
                "model": { "flag": "--model", "choices": [
                    { "id": "kept", "label": "Kept", "efforts": ["low"] },
                    { "id": "other", "label": "Other" }
                ] },
                "effort": { "flag": "--effort", "levels": [
                    { "id": "low", "label": "Low" },
                    { "id": "high", "label": "High" }
                ] }
            })
            .to_string(),
        )
        .expect("parses");
        let origin = |model: &str, effort: &str| SessionOrigin {
            model: Some(model.to_owned()),
            effort: Some(effort.to_owned()),
            forked_from: None,
        };
        let note = |what: &str, value: &str| {
            format!("The {what} {value} is no longer accepted by Fake, so this run did not use it.")
        };

        assert_eq!(
            kept_options(&decl, &origin("other", "low")),
            KeptOptions {
                model: Some("other".to_owned()),
                effort: Some("low".to_owned()),
                args: ["--model", "other", "--effort", "low"]
                    .map(str::to_owned)
                    .to_vec(),
                notes: Vec::new(),
            }
        );
        assert_eq!(
            kept_options(&decl, &origin("retired", "high")),
            KeptOptions {
                model: None,
                effort: Some("high".to_owned()),
                args: ["--effort", "high"].map(str::to_owned).to_vec(),
                notes: vec![note("model", "retired")],
            },
            "a model no choice lists is left out, and the effort is kept"
        );
        assert_eq!(
            kept_options(&decl, &origin("kept", "high")).notes,
            vec![note("effort", "high")],
            "the kept model lists only low"
        );
    }

    /// A fork's learned ref is dropped when it is its source's, so a fork never binds to the source's transcript; any
    /// other ref is kept, and so is any ref of a session that is no fork (decision 0168).
    #[test]
    fn a_fork_never_keeps_the_ref_of_its_source() {
        use super::release_tests::exited_row;
        let (agents, workspace_id, _profile) = service();
        let mut source = exited_row(&workspace_id, "terminal-source");
        source.agent_ref = Some("ref-source".to_owned());
        agents
            .store()
            .insert_agent_session_with_origin(&source, &SessionOrigin::default(), None)
            .unwrap();
        let fork = exited_row(&workspace_id, "terminal-fork");
        agents
            .store()
            .insert_agent_session_with_origin(
                &fork,
                &SessionOrigin {
                    forked_from: Some(source.id.clone()),
                    ..SessionOrigin::default()
                },
                None,
            )
            .unwrap();
        let fork = agents.store().get_agent_session(&fork.id).unwrap().unwrap();
        let learned = |text: &str| Some(text.to_owned());

        assert_eq!(
            agents
                .kept_learned_ref(&fork, learned("ref-source"))
                .unwrap(),
            None
        );
        assert_eq!(
            agents.kept_learned_ref(&fork, learned("ref-fork")).unwrap(),
            learned("ref-fork")
        );
        assert_eq!(
            agents
                .kept_learned_ref(&source, learned("ref-source"))
                .unwrap(),
            learned("ref-source"),
            "a session that is no fork keeps its ref"
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
                settle_until: None,
                switching: false,
                delivering: false,
                finished: false,
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
    pub(super) fn exited_row(workspace: &str, terminal: &str) -> AgentSessionRow {
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

    /// A Gentle Shell that only the registry's user `Path` lists (the bundled install, decision 0092) is listed as detected
    /// and starts on its launcher, as the extension's install check finds it (decision 0164). Without that folder it is
    /// neither detected nor startable.
    #[cfg(windows)]
    #[test]
    fn a_launcher_only_on_the_registry_user_path_is_listed_detected_and_started() {
        use std::ffi::OsString;

        use super::tests::service_searching;
        use crate::resolve::merge_search_path;

        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("bundle").join("bin");
        let process = dir.path().join("process");
        std::fs::create_dir_all(&bundle).unwrap();
        std::fs::create_dir_all(&process).unwrap();
        let launcher = bundle.join("gentle-shell.cmd");
        std::fs::write(&launcher, "@echo off\r\n").unwrap();
        let process_text = process.display().to_string();
        let user = bundle.display().to_string();
        let pathext = Some(".COM;.EXE;.BAT;.CMD".to_owned());
        let process_only = SearchEnv {
            path: Some(OsString::from(&process_text)),
            pathext: pathext.clone(),
        };
        let with_user_path = SearchEnv {
            path: Some(OsString::from(merge_search_path(
                &process_text,
                None,
                Some(&user),
            ))),
            pathext,
        };
        let is_launcher = |found: &str| found.eq_ignore_ascii_case(&launcher.to_string_lossy());

        let (without, _, _profile) = service_searching(
            Arc::new(crate::BuiltInPackages),
            VendorSearch::fixed(process_only),
        );
        let listed = without.kinds();
        let gentle = listed
            .kinds
            .iter()
            .find(|kind| kind.id == "gentle-shell")
            .expect("the built-in kind is listed");
        assert!(!gentle.detected, "{gentle:?}");
        assert!(without.startable_kind("gentle-shell").is_err());

        let (with, _, _profile) = service_searching(
            Arc::new(crate::BuiltInPackages),
            VendorSearch::fixed(with_user_path),
        );
        let listed = with.kinds();
        let gentle = listed
            .kinds
            .iter()
            .find(|kind| kind.id == "gentle-shell")
            .expect("the built-in kind is listed");
        assert!(gentle.detected, "{gentle:?}");
        assert!(
            gentle.program_path.as_deref().is_some_and(is_launcher),
            "{gentle:?}"
        );
        let (_, program) = with
            .startable_kind("gentle-shell")
            .expect("it starts on its launcher");
        assert!(is_launcher(&program.to_string_lossy()), "{program:?}");
    }
}

#[cfg(test)]
mod preview_tests {
    use super::*;

    #[test]
    fn a_preview_is_one_line_of_at_most_its_limit_and_absent_when_blank() {
        assert_eq!(
            preview_of("  done:\n  all   good \n"),
            Some("done: all good".to_owned())
        );
        assert_eq!(preview_of(" \n\t "), None);
        let long = "word ".repeat(100);
        let preview = preview_of(&long).unwrap();
        assert_eq!(preview.chars().count(), MAX_PREVIEW_CHARS);
        assert!(long.starts_with(&preview));
        let wide = "é".repeat(MAX_PREVIEW_CHARS + 5);
        assert_eq!(
            preview_of(&wide).unwrap().chars().count(),
            MAX_PREVIEW_CHARS
        );
    }
}
