//! The model catalog (decision 0166): the models a kind's picker offers for an account. The declared `model.choices` come
//! first, then the models the kind's vendor CLI lists for that account and the declaration does not name. A listing is
//! cached per (kind, account) for a while, and a failed listing is remembered for a shorter time.

use std::collections::{HashMap, HashSet};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use massorca_core_store::now_ms;
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    AccountState, AgentModel, AgentModels, ModelListingError, ModelListingFailure, ModelSource,
};

use crate::account_check::account_strip_names;
use crate::accounts::repair_state;
use crate::catalog::Catalog;
use crate::credentials;
use crate::kind::{DiscoverDecl, KindDecl, ListingFormat, ModelChoiceDecl, valid_option_value};
use crate::process_tree;
use crate::resolve::{SearchEnv, resolve_first};
use crate::sessions::{AgentError, Agents};

/// The most output a listing may print that is read; a longer one is cut off.
const LISTING_OUTPUT_LIMIT: u64 = 8 * 1024 * 1024;
/// How long the output is waited for once the listing's process tree is stopped.
const OUTPUT_GRACE: Duration = Duration::from_secs(1);
/// How often a running listing is checked against its deadline.
const POLL: Duration = Duration::from_millis(20);

/// How long a vendor CLI may take to list its models before the listing is stopped.
pub const LISTING_TIMEOUT: Duration = Duration::from_secs(15);
/// The environment variable that replaces [`LISTING_TIMEOUT`], in whole milliseconds, for tests.
pub const LISTING_TIMEOUT_ENV: &str = "MASSORCA_MODEL_LISTING_TIMEOUT_MS";
/// How long a fetched listing is served from the cache before the vendor CLI is asked again.
pub const LISTING_FRESH_MS: u64 = 10 * 60 * 1000;
/// How long a failed listing is served from the cache, so a burst of requests does not start the vendor CLI each time.
pub const FAILURE_FRESH_MS: u64 = 30 * 1000;
/// The most listings kept; the one fetched least recently goes first.
pub const MAX_LISTINGS: usize = 256;
/// The most characters kept from a listed label.
const MAX_LABEL_CHARS: usize = 120;
/// The most effort levels one listed model may name; the rest are dropped.
const MAX_LISTED_EFFORTS: usize = 16;

/// One model a listing named, before it is merged with the declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub id: String,
    pub label: String,
    /// The effort levels the listing gives the model: `Some(empty)` when it takes none, `None` when it names none.
    pub efforts: Option<Vec<String>>,
}

/// Reads a listing's output in its format. Only models whose id is a valid option value are kept, once each; anything
/// else in the output is skipped.
pub fn parse_listing(format: ListingFormat, stdout: &str) -> Vec<Listed> {
    let rows = match format {
        ListingFormat::Lines => lines_rows(stdout),
        ListingFormat::ClaudeControl => claude_control_rows(stdout),
    };
    let mut seen = HashSet::new();
    rows.into_iter()
        .filter(|row| seen.insert(row.id.clone()))
        .collect()
}

/// One row per `id<TAB>label` line. The header line is not a model.
fn lines_rows(stdout: &str) -> Vec<Listed> {
    stdout
        .lines()
        .filter_map(|line| {
            let (id, label) = line.split_once('\t')?;
            let id = id.trim();
            if matches!(id.to_ascii_lowercase().as_str(), "id" | "model") || !valid_option_value(id)
            {
                return None;
            }
            Some(Listed {
                id: id.to_owned(),
                label: clean_label(label, id),
                efforts: None,
            })
        })
        .collect()
}

/// The rows of the first stream-json `control_response` that answers `success` with a non-empty `models` list.
fn claude_control_rows(stdout: &str) -> Vec<Listed> {
    for line in stdout.lines().map(str::trim) {
        if !line.starts_with('{') || !line.contains("control_response") {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if parsed["type"] != "control_response" || parsed["response"]["subtype"] != "success" {
            continue;
        }
        let Some(models) = parsed["response"]["response"]["models"].as_array() else {
            continue;
        };
        let rows: Vec<Listed> = models.iter().filter_map(claude_row).collect();
        if !rows.is_empty() {
            return rows;
        }
    }
    Vec::new()
}

/// One picker row of a Claude listing. The `default` row mirrors whichever model is current, and a disabled row cannot be
/// started, so neither is listed. A model that takes effort names its levels, or none when it names none.
fn claude_row(entry: &serde_json::Value) -> Option<Listed> {
    let id = entry["value"].as_str()?.trim();
    if id == "default" || entry["disabled"] == true || !valid_option_value(id) {
        return None;
    }
    let efforts = if entry["supportsEffort"] == true {
        entry["supportedEffortLevels"].as_array().map(|levels| {
            levels
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter(|level| valid_option_value(level))
                .take(MAX_LISTED_EFFORTS)
                .map(str::to_owned)
                .collect()
        })
    } else {
        Some(Vec::new())
    };
    Some(Listed {
        id: id.to_owned(),
        label: clean_label(entry["displayName"].as_str().unwrap_or(""), id),
        efforts,
    })
}

/// The label kept from a listing, shortened to [`MAX_LABEL_CHARS`]; the id stands in for an empty one.
fn clean_label(label: &str, id: &str) -> String {
    let kept: String = label.trim().chars().take(MAX_LABEL_CHARS).collect();
    if kept.is_empty() { id.to_owned() } else { kept }
}

/// The declared models in order, then the listed ones the declaration does not name. A declared model keeps its label,
/// its default and its efforts; a listed model is never the default.
pub fn merge(declared: &[ModelChoiceDecl], listed: &[Listed]) -> Vec<AgentModel> {
    let mut models: Vec<AgentModel> = declared
        .iter()
        .map(|choice| AgentModel {
            id: choice.id.clone(),
            label: choice.label.clone(),
            default: choice.default,
            efforts: choice.efforts.clone(),
            source: ModelSource::Declared,
        })
        .collect();
    for item in listed {
        if models.iter().all(|model| model.id != item.id) {
            models.push(AgentModel {
                id: item.id.clone(),
                label: item.label.clone(),
                default: false,
                efforts: item.efforts.clone(),
                source: ModelSource::Discovered,
            });
        }
    }
    models
}

/// The (kind, account) a listing is kept for. `account` is `None` for the vendor's usual home.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ListingKey {
    pub kind: String,
    pub account: Option<String>,
}

/// A listing the vendor CLI answered, and when it did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    pub at_ms: u64,
    pub models: Vec<Listed>,
}

/// A listing that failed, and when it did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failed {
    pub at_ms: u64,
    pub error: ModelListingError,
}

/// What is known about one listing: the latest answer, and the latest failure when that came after the answer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    pub listed: Option<Fetched>,
    pub failed: Option<Failed>,
}

impl Entry {
    /// When the latest fetch of this listing happened, as an answer or a failure.
    fn touched_ms(&self) -> u64 {
        let answered = self.listed.as_ref().map(|listing| listing.at_ms);
        let failed = self.failed.as_ref().map(|failure| failure.at_ms);
        answered.max(failed).unwrap_or(0)
    }
}

/// The listings the service keeps, shared by every call, behind one lock. Held in memory only (decision 0166).
#[derive(Debug, Default)]
pub struct ModelCache {
    state: Mutex<CacheState>,
}

/// What the cache holds: the latest answer per listing, and the listings running now.
#[derive(Debug, Default)]
struct CacheState {
    entries: HashMap<ListingKey, Entry>,
    running: HashMap<ListingKey, Arc<Running>>,
}

/// A listing that is running. A call that finds it waits on the listing's condition variable for the answer, so waiting
/// costs no thread (decision 0166).
#[derive(Debug, Default)]
pub struct Running {
    /// The answer, once the listing has finished.
    answer: Mutex<Option<Entry>>,
    ready: Condvar,
    /// Set when the account or kind changed while the listing ran. Its answer still reaches the calls that wait for it,
    /// but nothing is cached from it.
    superseded: AtomicBool,
}

impl Running {
    /// The listing's answer, waiting until the listing has finished.
    pub fn wait(&self) -> Entry {
        let mut answer = self.answer.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if let Some(entry) = answer.as_ref() {
                return entry.clone();
            }
            answer = self
                .ready
                .wait(answer)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Hands the answer to every call that waits for the listing.
    fn settle(&self, entry: &Entry) {
        *self.answer.lock().unwrap_or_else(PoisonError::into_inner) = Some(entry.clone());
        self.ready.notify_all();
    }
}

/// What a call gets for one listing (decision 0166).
#[derive(Debug)]
pub enum Claim<'a> {
    /// A fresh answer is cached, so no vendor CLI runs.
    Fresh(Entry),
    /// A listing for the same key is running: its answer is this call's answer.
    Waiting(Arc<Running>),
    /// This call runs the listing, and reports its outcome with [`Leader::finish`].
    Lead(Leader<'a>),
}

/// The call that runs a listing. If it is dropped before it finishes, as by a panic, the listing settles with a failure, so
/// the calls waiting for it do not wait forever.
#[derive(Debug)]
pub struct Leader<'a> {
    cache: &'a ModelCache,
    key: ListingKey,
    running: Arc<Running>,
    started_ms: u64,
    finished: bool,
}

impl Leader<'_> {
    /// Reports the outcome to this call and to the calls waiting for the listing, and returns the answer.
    pub fn finish(mut self, outcome: Result<Vec<Listed>, ModelListingError>) -> Entry {
        self.finished = true;
        self.cache
            .settle(&self.key, &self.running, self.started_ms, outcome)
    }
}

impl Drop for Leader<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let error = listing_error(
                ModelListingFailure::Failed,
                "the listing ended without an answer",
            );
            self.cache
                .settle(&self.key, &self.running, self.started_ms, Err(error));
        }
    }
}

impl ModelCache {
    /// What a call gets for `key` at `now_ms`: the cached answer while it is fresh (unless `refresh` asks again), the answer
    /// of a listing already running for the key, or the lead of a new listing. Concurrent first requests share one listing.
    pub fn claim(&self, key: &ListingKey, now_ms: u64, refresh: bool) -> Claim<'_> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if !refresh && let Some(entry) = fresh_entry(state.entries.get(key), now_ms) {
            return Claim::Fresh(entry);
        }
        if let Some(running) = state.running.get(key) {
            return Claim::Waiting(Arc::clone(running));
        }
        let running = Arc::new(Running::default());
        state.running.insert(key.clone(), Arc::clone(&running));
        Claim::Lead(Leader {
            cache: self,
            key: key.clone(),
            running,
            started_ms: now_ms,
            finished: false,
        })
    }

    /// Drops the answer for `key`. A listing running for it still answers its own callers, but caches nothing.
    pub fn invalidate(&self, key: &ListingKey) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.entries.remove(key);
        if let Some(running) = state.running.remove(key) {
            running.superseded.store(true, Ordering::SeqCst);
        }
    }

    /// Drops the answer of `account` of `kind`: it was signed in, imported, removed, or made active (decision 0166).
    pub fn invalidate_account(&self, kind: &str, account: &str) {
        self.invalidate(&ListingKey {
            kind: kind.to_owned(),
            account: Some(account.to_owned()),
        });
    }

    /// Drops the answers a default request uses when the kind's active account moves from `previous` to `current`: the
    /// usual home's, and those of both accounts (decision 0166).
    pub fn invalidate_defaults(&self, kind: &str, previous: Option<&str>, current: Option<&str>) {
        self.invalidate(&ListingKey {
            kind: kind.to_owned(),
            account: None,
        });
        for account in [previous, current].into_iter().flatten() {
            self.invalidate_account(kind, account);
        }
    }

    /// Drops every answer and every running listing of `kind`, whatever its account: its package was removed (decision 0166).
    pub fn invalidate_kind(&self, kind: &str) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.entries.retain(|key, _| key.kind != kind);
        state.running.retain(|key, running| {
            let keep = key.kind != kind;
            if !keep {
                running.superseded.store(true, Ordering::SeqCst);
            }
            keep
        });
    }

    /// Stores the outcome of a listing unless the listing was superseded, and returns the answer for its callers.
    fn settle(
        &self,
        key: &ListingKey,
        running: &Arc<Running>,
        at_ms: u64,
        outcome: Result<Vec<Listed>, ModelListingError>,
    ) -> Entry {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = match outcome {
            Ok(models) => Entry {
                listed: Some(Fetched { at_ms, models }),
                failed: None,
            },
            Err(error) => Entry {
                listed: state
                    .entries
                    .get(key)
                    .and_then(|cached| cached.listed.clone()),
                failed: Some(Failed { at_ms, error }),
            },
        };
        if !running.superseded.load(Ordering::SeqCst) {
            state.entries.insert(key.clone(), entry.clone());
            evict_oldest(&mut state.entries);
        }
        if state
            .running
            .get(key)
            .is_some_and(|current| Arc::ptr_eq(current, running))
        {
            state.running.remove(key);
        }
        drop(state);
        running.settle(&entry);
        entry
    }
}

/// The entry when its latest fetch is still fresh at `now_ms`: a listing for [`LISTING_FRESH_MS`], or a failure for
/// [`FAILURE_FRESH_MS`] when the failure is the latest fetch.
fn fresh_entry(entry: Option<&Entry>, now_ms: u64) -> Option<Entry> {
    let entry = entry?;
    let (at_ms, window_ms) = match (&entry.failed, &entry.listed) {
        (Some(failed), listed)
            if listed
                .as_ref()
                .is_none_or(|listing| failed.at_ms >= listing.at_ms) =>
        {
            (failed.at_ms, FAILURE_FRESH_MS)
        }
        (_, Some(listing)) => (listing.at_ms, LISTING_FRESH_MS),
        _ => return None,
    };
    (now_ms.saturating_sub(at_ms) < window_ms).then(|| entry.clone())
}

#[cfg(test)]
impl ModelCache {
    /// The entry for `key` when its latest fetch is still fresh at `now_ms`.
    fn fresh(&self, key: &ListingKey, now_ms: u64) -> Option<Entry> {
        fresh_entry(self.get(key).as_ref(), now_ms)
    }

    /// The entry for `key`, however old its fetch is.
    fn get(&self, key: &ListingKey) -> Option<Entry> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entries
            .get(key)
            .cloned()
    }

    /// Stores a listing the vendor CLI answered; it clears an earlier failure.
    fn record_listing(&self, key: ListingKey, at_ms: u64, models: Vec<Listed>) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = state.entries.entry(key).or_default();
        entry.listed = Some(Fetched { at_ms, models });
        entry.failed = None;
        evict_oldest(&mut state.entries);
    }

    /// Stores a failed listing. The last answer, if there is one, is kept beside it.
    fn record_failure(&self, key: ListingKey, at_ms: u64, error: ModelListingError) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.entries.entry(key).or_default().failed = Some(Failed { at_ms, error });
        evict_oldest(&mut state.entries);
    }
}

/// Drops the listing fetched least recently while more than [`MAX_LISTINGS`] are held.
fn evict_oldest(entries: &mut HashMap<ListingKey, Entry>) {
    while entries.len() > MAX_LISTINGS {
        let oldest = entries
            .iter()
            .min_by_key(|(_, entry)| entry.touched_ms())
            .map(|(key, _)| key.clone());
        let Some(oldest) = oldest else {
            break;
        };
        entries.remove(&oldest);
    }
}

/// The result of `agent.models` for a kind: the merged models, the time of the listing they come from, and the latest
/// failure. `entry` is `None` when nothing has been listed for the account yet.
pub fn view(declared: &[ModelChoiceDecl], entry: Option<&Entry>) -> AgentModels {
    let listed = entry.and_then(|entry| entry.listed.as_ref());
    let listed_models = match listed {
        Some(listing) => listing.models.as_slice(),
        None => &[],
    };
    AgentModels {
        models: merge(declared, listed_models),
        fetched_at_ms: listed.map(|listing| listing.at_ms),
        error: entry
            .and_then(|entry| entry.failed.as_ref())
            .map(|failed| failed.error.clone()),
    }
}

/// The listing time limit for a value of [`LISTING_TIMEOUT_ENV`]: whole milliseconds above zero. Any other value, or none,
/// is [`LISTING_TIMEOUT`].
pub(crate) fn listing_timeout_from(value: Option<&str>) -> Duration {
    value
        .and_then(|text| text.trim().parse::<u64>().ok())
        .filter(|millis| *millis > 0)
        .map_or(LISTING_TIMEOUT, Duration::from_millis)
}

fn listing_error(code: ModelListingFailure, message: &str) -> ModelListingError {
    ModelListingError {
        code,
        message: message.to_owned(),
    }
}

impl Agents {
    /// The models `kind`'s picker offers for `account` (decision 0166): the declared choices, then what the kind's vendor
    /// CLI lists for the account. Without `account` the kind's active account is listed, or the vendor's usual home when
    /// the kind has none. A kind that declares no listing answers with its declared choices alone. The vendor CLI is
    /// asked only when the cache holds no fresh answer, or when `refresh` asks again.
    pub fn agent_models(
        &self,
        kind: &str,
        account: Option<&str>,
        refresh: bool,
    ) -> Result<AgentModels, AgentError> {
        self.require_enabled_kind(kind)?;
        let (decl, program) = self.listing_kind(kind)?;
        let declared = match &decl.model {
            Some(model) => model.choices.as_slice(),
            None => &[],
        };
        let Some(discover) = decl
            .model
            .as_ref()
            .and_then(|model| model.discover.as_ref())
        else {
            return Ok(view(declared, None));
        };
        let home = self.listing_home(&decl, kind, account)?;
        let key = ListingKey {
            kind: kind.to_owned(),
            account: home.as_ref().map(|(id, _)| id.clone()),
        };
        let now = u64::try_from(now_ms()).unwrap_or(0);
        let entry = match self.model_cache().claim(&key, now, refresh) {
            Claim::Fresh(entry) => entry,
            Claim::Waiting(running) => running.wait(),
            Claim::Lead(leader) => {
                let outcome = match program {
                    None => Err(listing_error(
                        ModelListingFailure::CliMissing,
                        "the kind's program was not found",
                    )),
                    Some(program) => self.list_models(&decl, &program, discover, home.as_ref()),
                };
                leader.finish(outcome)
            }
        };
        Ok(view(declared, Some(&entry)))
    }

    /// Drops the cached listings a change to `account` of `kind` can make wrong (decision 0166): the account's own, and the
    /// ones the default request uses now, which are the usual home's and the active account's.
    pub(crate) fn forget_listings(&self, kind: &str, account: &str) {
        let active = self.store().get_active_agent_account(kind).ok().flatten();
        self.model_cache().invalidate_account(kind, account);
        self.model_cache()
            .invalidate_defaults(kind, active.as_deref(), active.as_deref());
    }

    /// The kind's declaration and the program it runs, or `None` for the program when no candidate resolves on the
    /// service's `PATH`. The kind must be declared, valid and enabled, as a start requires.
    fn listing_kind(&self, kind: &str) -> Result<(KindDecl, Option<PathBuf>), AgentError> {
        let catalog = Catalog::load(self.kind_packages(), self.profile_dir());
        let entry = catalog.find(kind).ok_or_else(|| {
            AgentError::new(codes::AGENT_KIND_NOT_FOUND, format!("no agent kind {kind}"))
        })?;
        let decl = entry.valid().ok_or_else(|| {
            AgentError::new(
                codes::AGENT_NOT_INSTALLED,
                format!(
                    "agent kind {kind} is invalid: {}",
                    entry.problems.join("; ")
                ),
            )
        })?;
        let program = resolve_first(&decl.program, &SearchEnv::current());
        Ok((decl.clone(), program))
    }

    /// The account a listing runs for and its home (decision 0166), read as a run reads it: the named account, else the
    /// active one, else none. An account with no usable home is refused, and its home is repaired first (decision 0099).
    fn listing_home(
        &self,
        decl: &KindDecl,
        kind: &str,
        account: Option<&str>,
    ) -> Result<Option<(String, PathBuf)>, AgentError> {
        let id = match account {
            Some(id) => {
                self.account_of(kind, id)?;
                id.to_owned()
            }
            None => match self.store().get_active_agent_account(kind)? {
                Some(id) => id,
                None if self.store().kind_has_had_account(kind)? => {
                    return Err(AgentError::new(
                        codes::AGENT_CREDENTIALS_INVALID,
                        format!(
                            "the active account for {kind} was removed; choose or add an account"
                        ),
                    ));
                }
                None => return Ok(None),
            },
        };
        let _gate = credentials::lock_homes();
        let home = credentials::usable_account_home(self.profile_dir(), kind, &id)?.ok_or_else(
            || {
                AgentError::new(
                    codes::AGENT_CREDENTIALS_INVALID,
                    format!(
                        "the account of agent kind {kind} has no credential home; sign in again or remove it"
                    ),
                )
            },
        )?;
        repair_state(decl, &home)?;
        Ok(Some((id, home)))
    }

    /// Runs the kind's listing with the account's home variable set (decision 0166), then reads it. A listing that fails
    /// on an account its check says needs a sign-in is `not_signed_in`, so the picker can offer the sign-in command.
    fn list_models(
        &self,
        decl: &KindDecl,
        program: &Path,
        discover: &DiscoverDecl,
        home: Option<&(String, PathBuf)>,
    ) -> Result<Vec<Listed>, ModelListingError> {
        let home_var = decl
            .credentials
            .as_ref()
            .map(|credentials| credentials.home_env.as_str());
        let home_for_run = match (home, home_var) {
            (Some((_, path)), Some(name)) => Some((name, path.as_path())),
            _ => None,
        };
        let strip = account_strip_names(decl);
        let result = run_listing(
            program,
            discover,
            home_for_run,
            &strip,
            self.listing_timeout(),
        )
        .and_then(|stdout| {
            let listed = parse_listing(discover.format, &stdout);
            if listed.is_empty() {
                Err(listing_error(
                    ModelListingFailure::NoModels,
                    "the vendor CLI listed no model MassOrca can use",
                ))
            } else {
                Ok(listed)
            }
        });
        match (result, home) {
            (Err(error), Some((account, _)))
                if matches!(
                    error.code,
                    ModelListingFailure::Failed | ModelListingFailure::NoModels
                ) && self.needs_sign_in(decl, program, account) =>
            {
                Err(listing_error(
                    ModelListingFailure::NotSignedIn,
                    "the account is not signed in",
                ))
            }
            (result, _) => result,
        }
    }

    /// Whether the kind's check reports that `account` needs a sign-in. A kind that declares no check never does.
    fn needs_sign_in(&self, decl: &KindDecl, program: &Path, account: &str) -> bool {
        self.check_verdict(decl, program, account, None).state == AccountState::NeedsSignIn
    }
}

/// Runs `program` with the listing's arguments for at most `timeout` (decision 0166). The home variable is set to `home`
/// when there is one, and the names in `strip` are removed; the process tree is stopped before this returns. The listing
/// succeeds only when it exits with status 0; its stdout is the output.
fn run_listing(
    program: &Path,
    discover: &DiscoverDecl,
    home: Option<(&str, &Path)>,
    strip: &[String],
    timeout: Duration,
) -> Result<String, ModelListingError> {
    let mut command = massorca_process::command(program);
    command
        .args(&discover.args)
        .stdin(if discover.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some((name, path)) = home {
        command.env(name, path);
    }
    for name in strip {
        command.env_remove(name);
    }
    let (mut child, mut tree) = process_tree::spawn(&mut command).map_err(|error| {
        let code = if error.kind() == io::ErrorKind::NotFound {
            ModelListingFailure::CliMissing
        } else {
            ModelListingFailure::Failed
        };
        listing_error(code, "the vendor CLI could not be started")
    })?;
    if let (Some(mut pipe), Some(text)) = (child.stdin.take(), discover.stdin.clone()) {
        // Written on a thread of its own, so a CLI that never reads its stdin cannot hold up the deadline; dropping the
        // pipe closes it.
        thread::spawn(move || {
            let _ = pipe.write_all(text.as_bytes());
        });
    }
    let stdout = read_capped(child.stdout.take());
    drop(read_capped(child.stderr.take()));
    let exit = wait_until(&mut child, timeout);
    tree.stop();
    match exit {
        None => Err(listing_error(
            ModelListingFailure::TimedOut,
            "the vendor CLI did not list its models in time",
        )),
        Some(Some(0)) => Ok(stdout
            .recv_timeout(OUTPUT_GRACE)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default()),
        Some(_) => Err(listing_error(
            ModelListingFailure::Failed,
            "the vendor CLI exited without listing its models",
        )),
    }
}

/// Reads a stream to its end, or to [`LISTING_OUTPUT_LIMIT`], on a thread of its own. The bytes arrive on the channel.
fn read_capped<R: Read + Send + 'static>(stream: Option<R>) -> mpsc::Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(stream) = stream {
            let _ = stream.take(LISTING_OUTPUT_LIMIT).read_to_end(&mut bytes);
        }
        let _ = sender.send(bytes);
    });
    receiver
}

/// The child's exit code once it exits before `timeout`: the outer `None` when it ran past the deadline, and then it is
/// killed. The inner `None` is a child that ended with no code, such as one stopped by a signal.
fn wait_until(child: &mut Child, timeout: Duration) -> Option<Option<i32>> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status.code()),
            Ok(None) if started.elapsed() < timeout => thread::sleep(POLL),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(id: &str, label: &str, default: bool, efforts: Option<&[&str]>) -> ModelChoiceDecl {
        ModelChoiceDecl {
            id: id.to_owned(),
            label: label.to_owned(),
            default,
            efforts: efforts.map(|levels| levels.iter().map(|level| (*level).to_owned()).collect()),
        }
    }

    fn listed(id: &str) -> Vec<Listed> {
        vec![Listed {
            id: id.to_owned(),
            label: id.to_owned(),
            efforts: None,
        }]
    }

    fn key(kind: &str, account: Option<&str>) -> ListingKey {
        ListingKey {
            kind: kind.to_owned(),
            account: account.map(str::to_owned),
        }
    }

    fn failure(code: ModelListingFailure, message: &str) -> ModelListingError {
        ModelListingError {
            code,
            message: message.to_owned(),
        }
    }

}
