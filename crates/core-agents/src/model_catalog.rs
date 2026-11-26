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
    AccountState, AgentKind, AgentModel, AgentModels, KindModelChoice, ModelListingError,
    ModelListingFailure, ModelSource,
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
    /// The model the vendor says the id runs now, such as `claude-opus-5-5` for the alias `opus`; `None` when the listing
    /// names none (decision 0166, amendment).
    pub resolved: Option<String>,
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
                resolved: None,
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
        resolved: entry["resolvedModel"]
            .as_str()
            .map(str::trim)
            .filter(|resolved| !resolved.is_empty())
            .map(str::to_owned),
    })
}

/// The label kept from a listing, shortened to [`MAX_LABEL_CHARS`]; the id stands in for an empty one.
fn clean_label(label: &str, id: &str) -> String {
    let kept: String = label.trim().chars().take(MAX_LABEL_CHARS).collect();
    if kept.is_empty() { id.to_owned() } else { kept }
}

/// The label a resolved model id reads as (decision 0166, amendment): the family word title-cased, then the version
/// digits joined by dots, so `claude-opus-5-5` reads as `Opus 5.5`. The id must be `<vendor>-<family>-<n>[-<n>...]`,
/// with alphabetic vendor and family words and version numbers of one to three digits; a trailing eight-digit release
/// date and a bracketed variant such as `[1m]` are not part of the version. Any other shape reads as nothing, so the
/// declared label stands. No table names a version: the vendor's own answer is the only source.
pub fn versioned_label(resolved: &str) -> Option<String> {
    let base = match resolved.split_once('[') {
        Some((base, variant)) if variant.ends_with(']') => base,
        Some(_) => return None,
        None => resolved,
    };
    let mut parts: Vec<&str> = base.split('-').collect();
    if parts.len() > 3
        && parts
            .last()
            .is_some_and(|last| last.len() == 8 && last.bytes().all(|byte| byte.is_ascii_digit()))
    {
        parts.pop();
    }
    let [vendor, family, version @ ..] = parts.as_slice() else {
        return None;
    };
    let word = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_alphabetic());
    let number = |text: &&str| {
        (1..=3).contains(&text.len()) && text.bytes().all(|byte| byte.is_ascii_digit())
    };
    if !word(vendor) || !word(family) || version.is_empty() || !version.iter().all(number) {
        return None;
    }
    let lower = family.to_ascii_lowercase();
    let mut chars = lower.chars();
    let first = chars.next()?.to_ascii_uppercase();
    Some(format!("{first}{} {}", chars.as_str(), version.join(".")))
}

/// The label a declared choice shows: the versioned label of the model the listing resolves its id to, else its own.
fn declared_label(id: &str, label: &str, listed: &[Listed]) -> String {
    listed
        .iter()
        .find(|item| item.id == id)
        .and_then(|item| item.resolved.as_deref())
        .and_then(versioned_label)
        .unwrap_or_else(|| label.to_owned())
}

/// Gives `agent.kinds`' model choices the versioned labels of a listing (decision 0166, amendment); only labels change.
pub fn version_choices(choices: &mut [KindModelChoice], listed: &[Listed]) {
    for choice in choices {
        choice.label = declared_label(&choice.id, &choice.label, listed);
    }
}

/// The declared models in order, then the listed ones the declaration does not name. A declared model keeps its default
/// and its efforts, and its label unless the listing resolves its id to a versioned one; a listed model is never the
/// default and keeps the vendor's own label.
pub fn merge(declared: &[ModelChoiceDecl], listed: &[Listed]) -> Vec<AgentModel> {
    let mut models: Vec<AgentModel> = declared
        .iter()
        .map(|choice| AgentModel {
            id: choice.id.clone(),
            label: declared_label(&choice.id, &choice.label, listed),
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

    /// The models of the last listing the vendor CLI answered for `key`, however old; `None` when none answered. Reading
    /// it never starts a listing.
    pub fn last_listing(&self, key: &ListingKey) -> Option<Vec<Listed>> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let listed = state.entries.get(key)?.listed.as_ref()?;
        Some(listed.models.clone())
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

    /// Gives each kind's model choices the versioned labels of the catalog's last listing for the kind's default request
    /// (decision 0166, amendment): its active account, else the vendor's usual home when the kind has never had an
    /// account. Nothing is listed here, so a label turns versioned once a listing has answered and follows each refresh.
    pub(crate) fn version_kind_labels(&self, kinds: &mut [AgentKind]) {
        for kind in kinds {
            let Some(model) = kind.model.as_mut() else {
                continue;
            };
            let account = match self.store().get_active_agent_account(&kind.id) {
                Ok(Some(id)) => Some(id),
                Ok(None) if matches!(self.store().kind_has_had_account(&kind.id), Ok(false)) => {
                    None
                }
                _ => continue,
            };
            let key = ListingKey {
                kind: kind.id.clone(),
                account,
            };
            if let Some(listed) = self.model_cache().last_listing(&key) {
                version_choices(&mut model.choices, &listed);
            }
        }
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
            fast: false,
        }
    }

    fn listed(id: &str) -> Vec<Listed> {
        vec![Listed {
            id: id.to_owned(),
            label: id.to_owned(),
            efforts: None,
            resolved: None,
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

    #[test]
    fn lines_list_each_valid_model_once_with_its_label_or_its_id() {
        let stdout = "id\tlabel\n\
            gemini-4-argon-high\tGemini 4 Argon (High)\n\
            claude-x\t\n\
            bad id\tSpaces are not a value\n\
            -flag\tA flag\n\
            no tab here\n\
            gemini-4-argon-high\tDuplicate\n";
        assert_eq!(
            parse_listing(ListingFormat::Lines, stdout),
            vec![
                Listed {
                    id: "gemini-4-argon-high".to_owned(),
                    label: "Gemini 4 Argon (High)".to_owned(),
                    efforts: None,
                    resolved: None,
                },
                Listed {
                    id: "claude-x".to_owned(),
                    label: "claude-x".to_owned(),
                    efforts: None,
                    resolved: None,
                },
            ]
        );
    }

    #[test]
    fn claude_control_lists_the_picker_rows_with_the_efforts_they_take() {
        let stdout = concat!(
            "{\"type\":\"system\",\"subtype\":\"init\"}\n",
            "{\"type\":\"control_response\",\"response\":{\"subtype\":\"success\",",
            "\"request_id\":\"massorca-model-discovery\",\"response\":{\"models\":[",
            "{\"value\":\"default\",\"displayName\":\"Default\"},",
            "{\"value\":\"opus\",\"displayName\":\"Opus\",\"resolvedModel\":\"claude-opus-5-5\",\"supportsEffort\":true,",
            "\"supportedEffortLevels\":[\"low\",\"high\",\"turbo bad\"]},",
            "{\"value\":\"haiku\",\"displayName\":\"Haiku\",\"resolvedModel\":\" \",\"supportsEffort\":false},",
            "{\"value\":\"cc-update-required-1\",\"displayName\":\"Fable 5.1 (disabled)\",\"disabled\":true},",
            "{\"value\":\"opus[1m]\",\"displayName\":\"Opus (1M context)\"}",
            "]}}}\n"
        );
        assert_eq!(
            parse_listing(ListingFormat::ClaudeControl, stdout),
            vec![
                Listed {
                    id: "opus".to_owned(),
                    label: "Opus".to_owned(),
                    efforts: Some(vec!["low".to_owned(), "high".to_owned()]),
                    resolved: Some("claude-opus-5-5".to_owned()),
                },
                Listed {
                    id: "haiku".to_owned(),
                    label: "Haiku".to_owned(),
                    efforts: Some(Vec::new()),
                    resolved: None,
                },
            ],
            "the default row, the disabled row and the bracketed id (not a valid option value) are skipped; a blank              resolved model is none"
        );
    }

    #[test]
    fn a_claude_error_answer_or_a_line_that_is_not_json_lists_nothing() {
        let stdout = "{\"type\":\"control_response\",\"response\":{\"subtype\":\"error\"}}\n\
            control_response but not json\n";
        assert!(parse_listing(ListingFormat::ClaudeControl, stdout).is_empty());
    }

    #[test]
    fn a_resolved_model_id_reads_as_its_family_and_its_dotted_version() {
        let cases = [
            ("claude-opus-5-5", "Opus 5.5"),
            ("claude-sonnet-5-5", "Sonnet 5.5"),
            ("claude-fable-5-2", "Fable 5.2"),
            ("claude-sonnet-5", "Sonnet 5"),
            ("claude-haiku-4-5-20251001", "Haiku 4.5"),
            ("claude-opus-5-5[1m]", "Opus 5.5"),
            ("CLAUDE-OPUS-5-5", "Opus 5.5"),
        ];
        for (resolved, label) in cases {
            assert_eq!(
                versioned_label(resolved).as_deref(),
                Some(label),
                "{resolved}: a release date and a context variant are not part of the version"
            );
        }
    }

    #[test]
    fn a_resolved_model_id_of_another_shape_reads_as_nothing() {
        for resolved in [
            "",
            "opus",
            "claude-opus",
            "claude-opus-current",
            "cc-update-required-1",
            "claude-5-5",
            "claude-opus-5-x",
            "claude-opus-5--5",
            "claude-opus-1234",
            "claude-opus-20251001",
            "claude-opus-5-5[1m",
            "gpt-5.5",
            "claude-op us-5",
        ] {
            assert_eq!(versioned_label(resolved), None, "{resolved:?}");
        }
    }

    #[test]
    fn a_declared_model_the_listing_resolves_reads_as_its_versioned_label() {
        let declared = vec![
            choice("opus", "Opus", false, None),
            choice("haiku", "Haiku", false, None),
            choice("sonnet", "Sonnet", true, None),
        ];
        let listed = vec![
            Listed {
                id: "opus".to_owned(),
                label: "Opus".to_owned(),
                efforts: None,
                resolved: Some("claude-opus-5-5".to_owned()),
            },
            Listed {
                id: "haiku".to_owned(),
                label: "Haiku".to_owned(),
                efforts: None,
                resolved: Some("claude-haiku-current".to_owned()),
            },
            Listed {
                id: "claude-opus-4-1".to_owned(),
                label: "Opus 4.1 (legacy)".to_owned(),
                efforts: None,
                resolved: Some("claude-opus-4-1".to_owned()),
            },
        ];
        let labels: Vec<String> = merge(&declared, &listed)
            .into_iter()
            .map(|model| model.label)
            .collect();
        assert_eq!(
            labels,
            ["Opus 5.5", "Haiku", "Sonnet", "Opus 4.1 (legacy)"],
            "an unknown shape or no listing keeps the declared label; a discovered row keeps the vendor's own label"
        );
    }

    #[test]
    fn the_kinds_choices_take_the_versioned_label_of_the_last_listing() {
        let mut choices = vec![
            KindModelChoice {
                id: "opus".to_owned(),
                label: "Opus".to_owned(),
                default: false,
                efforts: None,
                fast: true,
            },
            KindModelChoice {
                id: "sonnet".to_owned(),
                label: "Sonnet".to_owned(),
                default: true,
                efforts: None,
                fast: false,
            },
        ];
        let listed = vec![Listed {
            id: "opus".to_owned(),
            label: "Opus".to_owned(),
            efforts: None,
            resolved: Some("claude-opus-5-5".to_owned()),
        }];
        version_choices(&mut choices, &listed);
        let labels: Vec<&str> = choices.iter().map(|choice| choice.label.as_str()).collect();
        assert_eq!(labels, ["Opus 5.5", "Sonnet"]);
        assert!(
            choices[0].fast && choices[1].default,
            "only the label changes"
        );
    }

    #[test]
    fn merge_keeps_the_declared_rows_and_adds_only_the_models_the_declaration_does_not_name() {
        let declared = vec![
            choice("opus", "Opus", false, None),
            choice("sonnet", "Sonnet", true, Some(&["low"])),
            choice("haiku", "Haiku", false, Some(&[])),
        ];
        let listed = vec![
            Listed {
                id: "sonnet".to_owned(),
                label: "Sonnet (listed)".to_owned(),
                efforts: Some(vec!["high".to_owned()]),
                resolved: None,
            },
            Listed {
                id: "opus-new".to_owned(),
                label: "Opus New".to_owned(),
                efforts: None,
                resolved: None,
            },
        ];
        let models = merge(&declared, &listed);
        let shape: Vec<(&str, ModelSource, bool)> = models
            .iter()
            .map(|model| (model.id.as_str(), model.source, model.default))
            .collect();
        assert_eq!(
            shape,
            [
                ("opus", ModelSource::Declared, false),
                ("sonnet", ModelSource::Declared, true),
                ("haiku", ModelSource::Declared, false),
                ("opus-new", ModelSource::Discovered, false),
            ]
        );
        assert_eq!(models[1].label, "Sonnet");
        assert_eq!(models[1].efforts, Some(vec!["low".to_owned()]));
    }

    #[test]
    fn a_listing_is_served_until_it_is_stale_and_no_account_answers_for_another() {
        let cache = ModelCache::default();
        cache.record_listing(key("claude", Some("a")), 1_000, listed("opus-a"));
        let hit = cache
            .fresh(&key("claude", Some("a")), 1_000 + LISTING_FRESH_MS - 1)
            .expect("a listing inside its window is served");
        assert_eq!(
            hit.listed.map(|listing| listing.models[0].id.clone()),
            Some("opus-a".to_owned())
        );
        assert!(
            cache
                .fresh(&key("claude", Some("a")), 1_000 + LISTING_FRESH_MS)
                .is_none(),
            "at the end of its window the vendor CLI is asked again"
        );
        assert!(cache.fresh(&key("claude", Some("b")), 1_001).is_none());
        assert!(cache.fresh(&key("claude", None), 1_001).is_none());
    }

    #[test]
    fn a_refresh_replaces_the_listing_and_a_failure_keeps_the_last_answer() {
        let cache = ModelCache::default();
        let usual = key("claude", None);
        cache.record_listing(usual.clone(), 0, listed("old"));
        cache.record_failure(
            usual.clone(),
            5_000,
            failure(ModelListingFailure::TimedOut, "late"),
        );
        let entry = cache
            .fresh(&usual, 5_001)
            .expect("a fresh failure is served");
        assert_eq!(
            entry.failed.map(|failed| failed.error.code),
            Some(ModelListingFailure::TimedOut)
        );
        assert_eq!(
            entry.listed.map(|listing| listing.models[0].id.clone()),
            Some("old".to_owned()),
            "the earlier answer rides along with the failure"
        );
        assert!(
            cache.fresh(&usual, 5_000 + FAILURE_FRESH_MS).is_none(),
            "a stale failure is asked again"
        );
        cache.record_listing(usual.clone(), 40_000, listed("new"));
        let entry = cache.fresh(&usual, 40_001).expect("a new listing is fresh");
        assert!(entry.failed.is_none(), "a listing clears the failure");
        assert_eq!(
            entry.listed.map(|listing| listing.models[0].id.clone()),
            Some("new".to_owned())
        );
    }

    #[test]
    fn a_failed_listing_with_no_answer_is_held_for_the_short_window_only() {
        let cache = ModelCache::default();
        let k = key("antigravity", None);
        cache.record_failure(
            k.clone(),
            0,
            failure(ModelListingFailure::CliMissing, "gone"),
        );
        assert!(cache.fresh(&k, FAILURE_FRESH_MS - 1).is_some());
        assert!(cache.fresh(&k, FAILURE_FRESH_MS).is_none());
        assert!(cache.get(&k).is_some(), "get reads an entry however old");
    }

    #[test]
    fn the_view_carries_the_fetch_time_and_the_latest_failure_beside_the_models() {
        let declared = [choice("sonnet", "Sonnet", true, None)];
        let nothing = view(&declared, None);
        assert_eq!(nothing.models.len(), 1);
        assert_eq!((nothing.fetched_at_ms, nothing.error), (None, None));

        let entry = Entry {
            listed: Some(Fetched {
                at_ms: 7,
                models: listed("opus-x"),
            }),
            failed: Some(Failed {
                at_ms: 9,
                error: failure(ModelListingFailure::Failed, "exit 2"),
            }),
        };
        let shown = view(&declared, Some(&entry));
        assert_eq!(shown.fetched_at_ms, Some(7));
        assert_eq!(
            shown.error.map(|error| error.code),
            Some(ModelListingFailure::Failed)
        );
        let ids: Vec<&str> = shown.models.iter().map(|model| model.id.as_str()).collect();
        assert_eq!(ids, ["sonnet", "opus-x"]);
    }

    #[test]
    fn an_account_change_drops_its_own_answer_and_the_default_ones_and_no_other() {
        let cache = ModelCache::default();
        let alpha = key("claude", Some("alpha"));
        let beta = key("claude", Some("beta"));
        let usual = key("claude", None);
        let other_kind = key("codex", Some("alpha"));
        for listing_key in [&alpha, &beta, &usual, &other_kind] {
            cache.record_listing(listing_key.clone(), 1_000, listed("opus"));
        }

        cache.invalidate_account("claude", "alpha");
        assert!(
            cache.get(&alpha).is_none(),
            "signed in, imported or removed: its answer goes"
        );
        assert!(cache.get(&beta).is_some() && cache.get(&usual).is_some());

        cache.invalidate_defaults("claude", Some("beta"), Some("gamma"));
        assert!(
            cache.get(&usual).is_none(),
            "the no-account answer goes when the active account moves"
        );
        assert!(
            cache.get(&beta).is_none(),
            "the previous active account's answer goes"
        );
        assert!(
            cache.get(&other_kind).is_some(),
            "another kind keeps its answers"
        );

        cache.record_listing(beta.clone(), 2_000, listed("opus"));
        cache.invalidate_kind("claude");
        assert!(
            cache.get(&beta).is_none(),
            "a removed package takes every answer of its kind"
        );
        assert!(cache.get(&other_kind).is_some());
    }

    #[test]
    fn a_listing_invalidated_while_it_runs_answers_its_callers_but_caches_nothing() {
        let cache = ModelCache::default();
        let alpha = key("claude", Some("alpha"));
        let Claim::Lead(leader) = cache.claim(&alpha, 1_000, false) else {
            panic!("the first call leads the listing");
        };
        cache.invalidate_account("claude", "alpha");
        let answer = leader.finish(Ok(listed("opus")));
        assert_eq!(
            answer.listed.map(|listing| listing.models[0].id.clone()),
            Some("opus".to_owned())
        );
        assert!(
            cache.get(&alpha).is_none(),
            "the answer of a superseded listing is not cached"
        );
        assert!(
            matches!(cache.claim(&alpha, 1_001, false), Claim::Lead(_)),
            "the next call lists again"
        );
    }

    #[test]
    fn concurrent_first_requests_share_the_listing_that_is_running() {
        let cache = ModelCache::default();
        let alpha = key("claude", Some("alpha"));
        let Claim::Lead(leader) = cache.claim(&alpha, 1_000, false) else {
            panic!("the first call leads the listing");
        };
        let (joined, joined_rx) = mpsc::channel();
        let answer = thread::scope(|scope| {
            let waiter = scope.spawn(|| {
                let Claim::Waiting(running) = cache.claim(&alpha, 1_001, false) else {
                    panic!("a call during the listing waits for it");
                };
                joined.send(()).expect("the test is listening");
                running.wait()
            });
            joined_rx.recv().expect("the waiter joined");
            leader.finish(Ok(listed("opus")));
            waiter.join().expect("the waiter finished")
        });
        assert_eq!(
            answer.listed.map(|listing| listing.models[0].id.clone()),
            Some("opus".to_owned())
        );
        assert!(
            matches!(cache.claim(&alpha, 1_002, false), Claim::Fresh(_)),
            "the shared answer is cached for the next call"
        );
    }

    #[test]
    fn a_listing_its_leader_abandons_answers_its_waiters_with_a_failure() {
        let cache = ModelCache::default();
        let alpha = key("claude", Some("alpha"));
        let Claim::Lead(leader) = cache.claim(&alpha, 1_000, false) else {
            panic!("the first call leads the listing");
        };
        let (joined, joined_rx) = mpsc::channel();
        let answer = thread::scope(|scope| {
            let waiter = scope.spawn(|| {
                let Claim::Waiting(running) = cache.claim(&alpha, 1_001, false) else {
                    panic!("a call during the listing waits for it");
                };
                joined.send(()).expect("the test is listening");
                running.wait()
            });
            joined_rx.recv().expect("the waiter joined");
            drop(leader);
            waiter.join().expect("the waiter finished")
        });
        assert_eq!(
            answer.failed.map(|failed| failed.error.code),
            Some(ModelListingFailure::Failed)
        );
    }

    #[test]
    fn a_refresh_lists_again_even_while_the_answer_is_fresh() {
        let cache = ModelCache::default();
        let alpha = key("claude", Some("alpha"));
        cache.record_listing(alpha.clone(), 1_000, listed("opus"));
        assert!(matches!(cache.claim(&alpha, 1_001, false), Claim::Fresh(_)));
        let Claim::Lead(leader) = cache.claim(&alpha, 1_001, true) else {
            panic!("a refresh runs the listing again");
        };
        leader.finish(Ok(listed("sonnet")));
        assert_eq!(
            cache
                .get(&alpha)
                .and_then(|entry| entry.listed)
                .map(|listing| listing.models[0].id.clone()),
            Some("sonnet".to_owned())
        );
    }

    #[test]
    fn the_listing_time_limit_is_read_in_milliseconds_and_defaults_otherwise() {
        assert_eq!(
            listing_timeout_from(Some("500")),
            Duration::from_millis(500)
        );
        assert_eq!(
            listing_timeout_from(Some(" 250 ")),
            Duration::from_millis(250)
        );
        assert_eq!(listing_timeout_from(Some("0")), LISTING_TIMEOUT);
        assert_eq!(listing_timeout_from(Some("soon")), LISTING_TIMEOUT);
        assert_eq!(listing_timeout_from(None), LISTING_TIMEOUT);
    }
}
