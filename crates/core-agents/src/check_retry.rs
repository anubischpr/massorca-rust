//! Background retries of a program run stopped at its time limit (decision 0059, amended 2026-11-27): an account check
//! and a kind's version probe. A run killed at the cap is not a verdict. An account keeps its last known state, and a kind
//! its chat support as `version_unknown`, while the run is retried after each wait of [`CHECK_RETRY_DELAYS`], one retry
//! at a time per account or kind, until one run decides.
//!
//! - An account check's retry applies its result through the same path as `agent.accounts.check`, which announces a
//!   changed state with `agent.accounts_changed`. A newer explicit check supersedes a waiting retry, and a removal cancels
//!   it.
//! - A version probe's retry announces the kind ([`Change::Models`], published as `agent.accounts_changed` of the kind)
//!   once it decides, so a client reads the kinds again. A chat start that finds the probe pending waits for it, bounded,
//!   before it takes any gate; under a gate the support is read from the cache, a cached timeout included, with no wait.
//!
//! A decided verdict is never retried.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use massorca_protocol::methods::chat::KindChat;

use crate::account_check::{CHECK_TIMEOUT, CheckRun, run_check};
use crate::chat::version::kind_chat_with;
use crate::kind::KindDecl;
use crate::probe::PROBE_TIMEOUT;
use crate::sessions::{AgentError, Agents, Change, WeakAgents};

/// The waits before each retry of a timed-out run: three retries, the last about 22 seconds after the first run.
pub const CHECK_RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(15),
];

/// How often a waiting retry, or a chat start waiting for a probe, looks again.
const RETRY_POLL: Duration = Duration::from_millis(25);

/// A stand-in for a checked or probed program, given the program and its arguments: tests use it to make a run finish or
/// time out on cue.
pub(crate) type CheckRunner = Arc<dyn Fn(&Path, &[String]) -> CheckRun + Send + Sync>;

/// How account checks and version probes run.
#[derive(Clone)]
pub(crate) struct CheckSettings {
    /// How long an account check may run before it is stopped.
    pub(crate) timeout: Duration,
    /// How long a version probe may run before it is stopped.
    pub(crate) probe_timeout: Duration,
    /// The waits before each retry of a timed-out run; empty retries nothing.
    pub(crate) retry_delays: Vec<Duration>,
    /// Runs instead of the checked or probed program, when set.
    pub(crate) runner: Option<CheckRunner>,
}

impl Default for CheckSettings {
    fn default() -> Self {
        Self {
            timeout: CHECK_TIMEOUT,
            probe_timeout: PROBE_TIMEOUT,
            retry_delays: CHECK_RETRY_DELAYS.to_vec(),
            runner: None,
        }
    }
}

/// What a retry runs again.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum RetryKey {
    /// The check of an account, by id.
    Account(String),
    /// The version probe of a kind, by id.
    Probe(String),
}

/// One run of a retry, given the service and the retry's generation: `Ok(true)` when it timed out again. An error ends
/// the retries.
type Attempt = Box<dyn Fn(&Agents, u64) -> Result<bool, AgentError> + Send>;

/// The retries waiting or running: the generation of each current retry, and how many retry threads exist.
#[derive(Debug, Default)]
pub(crate) struct CheckRetries {
    current: HashMap<RetryKey, u64>,
    next: u64,
    threads: usize,
}

impl Agents {
    /// Runs one account check: the kind's program with the account's home, for at most the check time limit.
    pub(crate) fn run_account_check(
        &self,
        program: &Path,
        args: &[String],
        home: &Path,
        home_env: &str,
        strip_env: &[String],
    ) -> CheckRun {
        let settings = self.check_settings();
        if let Some(runner) = &settings.runner {
            return runner(program, args);
        }
        run_check(program, args, home, home_env, strip_env, settings.timeout)
    }

    /// Retries the check of `account` in the background, after a check of it timed out. A retry already waiting for the
    /// account is superseded by this one, so at most one is in flight.
    pub(crate) fn schedule_check_retry(&self, kind: &str, account: &str) {
        let (kind, id) = (kind.to_owned(), account.to_owned());
        self.schedule_retry(
            RetryKey::Account(account.to_owned()),
            true,
            Box::new(move |agents, generation| agents.check_for_retry(&kind, &id, generation)),
        );
    }

    /// Retries the version probe of `kind` in the background, after a probe of it timed out. A retry already in flight for
    /// the kind is kept, so at most one is.
    pub(crate) fn schedule_probe_retry(&self, kind: &str) {
        let id = kind.to_owned();
        self.schedule_retry(
            RetryKey::Probe(kind.to_owned()),
            false,
            Box::new(move |agents, _| agents.probe_for_retry(&id)),
        );
    }

    /// The chat support of `decl` with its resolved `program`, as a chat start reads it before any gate: a probe stopped at
    /// its time limit is retried in the background, and this waits for that retry, at most [`probe_wait`], before reading
    /// the support again. It can wait for seconds, so it is never called under a gate; a gated read is
    /// [`Self::chat_support_now`]. `None` for a kind without a `chat` block.
    pub(crate) fn chat_support(&self, decl: &KindDecl, program: &Path) -> Option<KindChat> {
        let settings = self.check_settings();
        let (chat, timed_out) = probe_chat(decl, program, &settings, false)?;
        if !timed_out {
            return Some(chat);
        }
        self.schedule_probe_retry(&decl.id);
        let deadline = Instant::now() + probe_wait(&settings);
        let key = RetryKey::Probe(decl.id.clone());
        while self.check_retries().current.contains_key(&key) && Instant::now() < deadline {
            thread::sleep(RETRY_POLL);
        }
        // The retry's answer, or its last timeout, is cached: this look runs no program.
        probe_chat(decl, program, &settings, false).map(|(chat, _)| chat)
    }

    /// The chat support of `decl` with its resolved `program`, as a read under a gate takes it: what the cache holds, a
    /// cached timeout included, with no wait. A caller that waited with [`Self::chat_support`] before its gate finds the
    /// cache filled. A timed-out probe is retried in the background, as `agent.kinds` retries it.
    pub(crate) fn chat_support_now(&self, decl: &KindDecl, program: &Path) -> Option<KindChat> {
        let settings = self.check_settings();
        let (chat, timed_out) = probe_chat(decl, program, &settings, false)?;
        if timed_out {
            self.schedule_probe_retry(&decl.id);
        }
        Some(chat)
    }

    /// One retry of a kind's timed-out version probe. `Ok(true)` when it timed out again; a decided probe announces the
    /// kind, so a client reads the kinds again. It runs the program whatever is cached, since the cache holds the timeout.
    fn probe_for_retry(&self, kind: &str) -> Result<bool, AgentError> {
        let (decl, program) = self.runnable_kind(kind)?;
        match probe_chat(&decl, &program, &self.check_settings(), true) {
            Some((_, true)) => Ok(true),
            Some((_, false)) => {
                self.notify(Change::Models(kind.to_owned()));
                Ok(false)
            }
            None => Ok(false),
        }
    }

    /// Starts a retry thread for `key`. With `supersede`, it replaces a retry already in flight; without, an existing one
    /// is kept and nothing starts.
    fn schedule_retry(&self, key: RetryKey, supersede: bool, attempt: Attempt) {
        let delays = self.check_settings().retry_delays;
        if delays.is_empty() {
            return;
        }
        let generation = {
            let mut retries = self.check_retries();
            if !supersede && retries.current.contains_key(&key) {
                return;
            }
            retries.next += 1;
            let generation = retries.next;
            retries.current.insert(key.clone(), generation);
            retries.threads += 1;
            generation
        };
        let weak = self.downgrade();
        let spawned = thread::Builder::new()
            .name("check-retry".to_owned())
            .spawn({
                let weak = weak.clone();
                let key = key.clone();
                move || retry_loop(&weak, &key, generation, &delays, &attempt)
            });
        if spawned.is_err() {
            tracing::warn!("a background retry could not start");
            if let Some(agents) = weak.upgrade() {
                agents.end_retry(&key, generation);
            }
        }
    }

    /// Cancels a retry waiting for `account`, if any: the account was removed, or is being checked again.
    pub(crate) fn cancel_check_retry(&self, account: &str) {
        self.check_retries()
            .current
            .remove(&RetryKey::Account(account.to_owned()));
    }

    /// Whether `generation` is still the retry wanted for `account`.
    pub(crate) fn retry_is_current(&self, account: &str, generation: u64) -> bool {
        self.key_is_current(&RetryKey::Account(account.to_owned()), generation)
    }

    fn key_is_current(&self, key: &RetryKey, generation: u64) -> bool {
        self.check_retries().current.get(key) == Some(&generation)
    }

    /// How many retry threads exist, waiting or running.
    #[cfg(test)]
    pub(crate) fn check_retry_threads(&self) -> usize {
        self.check_retries().threads
    }

    /// Ends a retry thread: its entry goes when it is still this retry's.
    fn end_retry(&self, key: &RetryKey, generation: u64) {
        let mut retries = self.check_retries();
        if retries.current.get(key) == Some(&generation) {
            retries.current.remove(key);
        }
        retries.threads = retries.threads.saturating_sub(1);
    }
}

/// The chat support of `decl` with `program`, probed with `settings`; `fresh` runs the program whatever is cached.
fn probe_chat(
    decl: &KindDecl,
    program: &Path,
    settings: &CheckSettings,
    fresh: bool,
) -> Option<(KindChat, bool)> {
    kind_chat_with(
        decl,
        Some(program),
        settings.probe_timeout,
        settings.runner.as_ref(),
        fresh,
    )
}

/// How long a chat start waits for a pending probe: every retry's wait, and a probe's time limit for each run.
fn probe_wait(settings: &CheckSettings) -> Duration {
    let runs = u32::try_from(settings.retry_delays.len()).unwrap_or(u32::MAX);
    settings.retry_delays.iter().sum::<Duration>()
        + settings
            .probe_timeout
            .saturating_mul(runs.saturating_add(1))
}

/// A retry thread's hold on its entry: dropped when the thread ends, however it ends (a panicking run included), it ends
/// the retry, so no entry and no thread count is left behind.
struct RetryEntry<'a> {
    weak: &'a WeakAgents,
    key: &'a RetryKey,
    generation: u64,
}

impl Drop for RetryEntry<'_> {
    fn drop(&mut self) {
        if let Some(agents) = self.weak.upgrade() {
            agents.end_retry(self.key, self.generation);
        }
    }
}

/// The body of a retry thread: after each wait, one run of `attempt`, until a run decides, the retries run out, or the
/// retry is cancelled or superseded. A refused run (the account or kind is gone, busy, or being removed) ends it.
fn retry_loop(
    weak: &WeakAgents,
    key: &RetryKey,
    generation: u64,
    delays: &[Duration],
    attempt: &Attempt,
) {
    let _entry = RetryEntry {
        weak,
        key,
        generation,
    };
    for delay in delays {
        if !wait_while_current(weak, key, generation, *delay) {
            break;
        }
        let Some(agents) = weak.upgrade() else {
            return;
        };
        match attempt(&agents, generation) {
            Ok(true) => tracing::info!(?key, "a run timed out again; it is retried later"),
            Ok(false) => break,
            Err(error) => {
                tracing::info!(?key, code = error.code, "a background retry ended");
                break;
            }
        }
    }
}

/// Waits `delay`, and says whether the retry is still wanted at its end. Returns early once it is not.
fn wait_while_current(weak: &WeakAgents, key: &RetryKey, generation: u64, delay: Duration) -> bool {
    let due = Instant::now() + delay;
    loop {
        match weak.upgrade() {
            Some(agents) if agents.key_is_current(key, generation) => {}
            _ => return false,
        }
        let now = Instant::now();
        if now >= due {
            return true;
        }
        thread::sleep(RETRY_POLL.min(due - now));
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use massorca_protocol::methods::agents::{AccountState, ImportResult};
    use massorca_protocol::methods::chat::ChatUnsupportedReason;

    use super::*;
    use crate::account_check::RunOutput;
    use crate::accounts::tests::{account_events, collect_changes, source_with_token};
    use crate::paths;
    use crate::sessions::tests::service;

    /// What one run of the stand-in check does.
    #[derive(Clone, Copy)]
    enum Step {
        /// Still running at the time limit.
        Hang,
        /// Signed in as `fake@example.test`.
        SignedIn,
        /// Finished and reports not signed in.
        SignedOut,
        /// Prints a CLI version inside the Claude chat range.
        Version,
        /// Panics, as a bug in the run would.
        Panic,
    }

    /// What one run of the stand-in check returns for `step`.
    fn run_of(step: Step) -> CheckRun {
        let (exit, output, timed_out) = match step {
            Step::Hang => (None, "", true),
            Step::SignedIn => (
                Some(0),
                r#"{"loggedIn":true,"email":"fake@example.test"}"#,
                false,
            ),
            Step::SignedOut => (Some(1), r#"{"loggedIn":false}"#, false),
            Step::Version => (Some(0), "2.1.300 (Claude Code)", false),
            Step::Panic => panic!("a run panics on cue"),
        };
        CheckRun {
            run: RunOutput {
                exit,
                output: output.to_owned(),
            },
            timed_out,
        }
    }

    /// A stand-in check that takes `steps` in order, then repeats the last, and counts its runs.
    fn scripted(steps: &[Step]) -> (CheckRunner, Arc<AtomicUsize>) {
        let steps = steps.to_vec();
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&runs);
        let runner: CheckRunner = Arc::new(move |_program, _args| {
            let index = counter.fetch_add(1, Ordering::SeqCst);
            run_of(steps[index.min(steps.len() - 1)])
        });
        (runner, runs)
    }

    /// Declares kind `fake` with a JSON check, run by this test binary (the stand-in replaces it), and makes the service
    /// use `runner` with a short time limit and `delays` between retries.
    fn fake_kind(agents: &Agents, profile: &Path, runner: CheckRunner, delays: &[Duration]) {
        let kinds = paths::kinds_dir(profile);
        fs::create_dir_all(&kinds).expect("create the kinds directory");
        let program = std::env::current_exe().expect("the test binary");
        let kind = serde_json::json!({
            "id": "fake",
            "name": "Fake",
            "program": [program.to_str().expect("a UTF-8 path")],
            "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"] },
            "accounts": { "check": {
                "args": ["auth", "status", "--json"],
                "identity_json": ["email"],
                "signed_in_json": "loggedIn"
            } }
        });
        fs::write(kinds.join("fake.json"), kind.to_string()).expect("write the fake kind");
        agents.set_check_settings(CheckSettings {
            timeout: Duration::from_millis(50),
            probe_timeout: Duration::from_millis(50),
            retry_delays: delays.to_vec(),
            runner: Some(runner),
        });
    }

    /// Waits until no retry thread is left.
    fn retries_settle(agents: &Agents) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while agents.check_retry_threads() > 0 {
            assert!(Instant::now() < deadline, "the retries did not end");
            thread::sleep(Duration::from_millis(10));
        }
    }

    const QUICK: [Duration; 3] = [Duration::from_millis(10); 3];

    #[test]
    fn a_timed_out_import_is_kept_unknown_and_a_retry_signs_it_in() {
        let (agents, _, profile) = service();
        let (runner, runs) = scripted(&[Step::Hang, Step::SignedIn]);
        fake_kind(&agents, profile.path(), runner, &QUICK);
        let changes = collect_changes(&agents);
        let source = source_with_token();

        let imported = agents
            .accounts_import("fake", source.path().to_str(), None)
            .expect("the import runs");
        assert_eq!(imported.result, ImportResult::Imported, "{imported:?}");
        let account = imported.account.expect("the import keeps its account");
        assert_eq!(
            account.state,
            AccountState::Unknown,
            "a timeout is no verdict"
        );

        retries_settle(&agents);
        let listed = agents.accounts_list("fake").expect("a list");
        assert_eq!(listed.len(), 1, "{listed:?}");
        assert_eq!(
            listed[0].state,
            AccountState::SignedIn,
            "with no explicit check"
        );
        assert_eq!(listed[0].identity.as_deref(), Some("fake@example.test"));
        assert_eq!(
            runs.load(Ordering::SeqCst),
            2,
            "the import's check and one retry"
        );
        let events = account_events(&changes);
        assert_eq!(
            events
                .last()
                .map(|event| (event.account.as_str(), event.state)),
            Some((account.id.as_str(), AccountState::SignedIn)),
            "{events:?}"
        );
    }

    #[test]
    fn a_check_that_times_out_every_time_stays_unknown_and_its_retries_stop() {
        let (agents, _, profile) = service();
        let (runner, runs) = scripted(&[Step::Hang]);
        fake_kind(&agents, profile.path(), runner, &QUICK);
        let source = source_with_token();

        let imported = agents
            .accounts_import("fake", source.path().to_str(), None)
            .expect("the import runs");
        assert_eq!(imported.result, ImportResult::Imported, "{imported:?}");

        retries_settle(&agents);
        assert_eq!(
            runs.load(Ordering::SeqCst),
            4,
            "the check and its three retries"
        );
        let listed = agents.accounts_list("fake").expect("a list");
        assert_eq!(listed[0].state, AccountState::Unknown);
    }

    #[test]
    fn a_decided_not_signed_in_is_not_retried() {
        let (agents, _, profile) = service();
        let (runner, runs) = scripted(&[Step::SignedIn, Step::SignedOut]);
        fake_kind(&agents, profile.path(), runner, &QUICK);
        let source = source_with_token();
        let account = agents
            .accounts_import("fake", source.path().to_str(), None)
            .expect("the import runs")
            .account
            .expect("kept");

        let checked = agents
            .accounts_check("fake", Some(&account.id))
            .expect("the check runs");

        assert_eq!(checked.state, AccountState::NeedsSignIn);
        assert_eq!(agents.check_retry_threads(), 0, "no retry was scheduled");
        assert_eq!(runs.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_removed_accounts_pending_retry_does_nothing() {
        let (agents, _, profile) = service();
        let (runner, runs) = scripted(&[Step::Hang, Step::SignedIn]);
        // A long wait: the removal comes well before the retry would run.
        fake_kind(&agents, profile.path(), runner, &[Duration::from_secs(30)]);
        let source = source_with_token();
        let account = agents
            .accounts_import("fake", source.path().to_str(), None)
            .expect("the import runs")
            .account
            .expect("kept");
        assert_eq!(agents.check_retry_threads(), 1, "a retry waits");
        let changes = collect_changes(&agents);

        agents
            .accounts_remove("fake", &account.id)
            .expect("the removal runs");

        retries_settle(&agents);
        assert_eq!(runs.load(Ordering::SeqCst), 1, "the retry never ran");
        let events = account_events(&changes);
        assert_eq!(events.len(), 1, "only the removal: {events:?}");
    }

    #[test]
    fn a_newer_explicit_check_supersedes_a_pending_retry() {
        let (agents, _, profile) = service();
        let (runner, runs) = scripted(&[Step::Hang, Step::SignedIn]);
        fake_kind(&agents, profile.path(), runner, &[Duration::from_secs(30)]);
        let source = source_with_token();
        let account = agents
            .accounts_import("fake", source.path().to_str(), None)
            .expect("the import runs")
            .account
            .expect("kept");

        let checked = agents
            .accounts_check("fake", Some(&account.id))
            .expect("the check runs");

        assert_eq!(checked.state, AccountState::SignedIn);
        retries_settle(&agents);
        assert_eq!(
            runs.load(Ordering::SeqCst),
            2,
            "the superseded retry never ran"
        );
    }

    #[test]
    fn a_timed_out_explicit_check_keeps_the_last_known_state() {
        let (agents, _, profile) = service();
        let (runner, runs) = scripted(&[Step::SignedIn, Step::Hang, Step::SignedIn]);
        fake_kind(&agents, profile.path(), runner, &QUICK);
        let source = source_with_token();
        let account = agents
            .accounts_import("fake", source.path().to_str(), None)
            .expect("the import runs")
            .account
            .expect("kept");
        assert_eq!(account.state, AccountState::SignedIn);
        let changes = collect_changes(&agents);

        let checked = agents
            .accounts_check("fake", Some(&account.id))
            .expect("the check runs");

        assert_eq!(checked.state, AccountState::SignedIn, "not overwritten");
        retries_settle(&agents);
        assert_eq!(runs.load(Ordering::SeqCst), 3, "one retry decided");
        assert!(
            account_events(&changes).is_empty(),
            "nothing changed, so nothing is announced"
        );
    }

    /// Declares kind `fakechat`, a chat kind whose program is a file of its own in the profile, so its version probe is
    /// cached apart from every other test's, and makes the service use `runner` with `delays` between retries.
    fn fake_chat_kind(
        agents: &Agents,
        profile: &Path,
        runner: CheckRunner,
        delays: &[Duration],
    ) -> std::path::PathBuf {
        let program = profile.join(format!("fake-chat-cli{}", std::env::consts::EXE_SUFFIX));
        fs::write(&program, b"not run: a stand-in answers").expect("the program file");
        let runner = only(&program, runner);
        let kinds = paths::kinds_dir(profile);
        fs::create_dir_all(&kinds).expect("create the kinds directory");
        let kind = serde_json::json!({
            "id": "fakechat",
            "name": "Fake Chat",
            "program": [program.to_str().expect("a UTF-8 path")],
            "prompt": { "mode": "none" },
            "session_ref": { "assign": ["--session-id", "{ref}"] },
            "resume": ["--resume", "{ref}"],
            "chat": { "adapter": "claude-stream-json", "min_version": "2.1.258", "max_major": 2 }
        });
        fs::write(kinds.join("fakechat.json"), kind.to_string()).expect("write the chat kind");
        agents.set_check_settings(CheckSettings {
            timeout: Duration::from_millis(50),
            probe_timeout: Duration::from_millis(50),
            retry_delays: delays.to_vec(),
            runner: Some(runner),
        });
        program
    }

    /// `runner` for `program` alone: any other program, such as a built-in kind's CLI found on this host, is not run.
    fn only(program: &Path, runner: CheckRunner) -> CheckRunner {
        let program = program.to_path_buf();
        Arc::new(move |asked: &Path, args: &[String]| {
            if asked == program {
                return runner(asked, args);
            }
            CheckRun {
                run: RunOutput {
                    exit: None,
                    output: String::new(),
                },
                timed_out: false,
            }
        })
    }

    /// The chat support `agent.kinds` lists for `fakechat`.
    fn listed_chat(agents: &Agents) -> KindChat {
        agents
            .kinds()
            .kinds
            .into_iter()
            .find(|kind| kind.id == "fakechat")
            .and_then(|kind| kind.chat)
            .expect("the chat kind is listed with its chat support")
    }

    /// Every change collected so far.
    fn all_changes(agents: &Agents) -> Arc<std::sync::Mutex<Vec<Change>>> {
        let changes = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = Arc::clone(&changes);
        agents.set_listener(Box::new(move |change| sink.lock().unwrap().push(change)));
        changes
    }

    #[test]
    fn a_timed_out_version_probe_is_no_verdict_and_a_retry_announces_the_kind() {
        let (agents, _, profile) = service();
        let (runner, runs) = scripted(&[Step::Hang, Step::Version]);
        fake_chat_kind(&agents, profile.path(), runner, &QUICK);
        let changes = all_changes(&agents);

        let first = listed_chat(&agents);
        assert!(!first.supported, "{first:?}");
        assert_eq!(first.reason, Some(ChatUnsupportedReason::VersionUnknown));

        retries_settle(&agents);
        assert!(
            changes
                .lock()
                .unwrap()
                .contains(&Change::Models("fakechat".to_owned())),
            "the kind is announced once the retry decides"
        );
        let after = listed_chat(&agents);
        assert!(after.supported, "{after:?}");
        assert_eq!(after.version.as_deref(), Some("2.1.300"));
        assert_eq!(
            runs.load(Ordering::SeqCst),
            2,
            "the retry's answer is cached"
        );
    }

    #[test]
    fn one_probe_retry_per_kind_is_in_flight() {
        let (agents, _, profile) = service();
        let (runner, _runs) = scripted(&[Step::Hang]);
        fake_chat_kind(&agents, profile.path(), runner, &[Duration::from_secs(30)]);

        listed_chat(&agents);
        listed_chat(&agents);

        assert_eq!(agents.check_retry_threads(), 1);
    }

    #[test]
    fn a_chat_start_waits_for_a_pending_probe_instead_of_refusing() {
        let (agents, _, profile) = service();
        let (runner, runs) = scripted(&[Step::Hang, Step::Version]);
        fake_chat_kind(&agents, profile.path(), runner, &QUICK);
        // The stand-in answers at once, so a long limit only widens the wait's bound: on a loaded host the retry thread
        // may start late, and the wait ends as soon as the retry does.
        let mut settings = agents.check_settings();
        settings.probe_timeout = Duration::from_secs(5);
        agents.set_check_settings(settings);
        let (decl, program) = agents.runnable_kind("fakechat").expect("the kind runs");

        let chat = agents.chat_support(&decl, &program).expect("a chat block");

        assert!(chat.supported, "{chat:?}");
        assert_eq!(
            runs.load(Ordering::SeqCst),
            2,
            "the start's probe and one retry"
        );
    }

    #[test]
    fn a_chat_start_stops_waiting_once_every_probe_timed_out() {
        let (agents, _, profile) = service();
        let (runner, runs) = scripted(&[Step::Hang]);
        fake_chat_kind(&agents, profile.path(), runner, &QUICK);
        let (decl, program) = agents.runnable_kind("fakechat").expect("the kind runs");
        let started = Instant::now();

        let chat = agents.chat_support(&decl, &program).expect("a chat block");

        assert_eq!(chat.reason, Some(ChatUnsupportedReason::VersionUnknown));
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the wait is bounded"
        );
        retries_settle(&agents);
        assert_eq!(
            runs.load(Ordering::SeqCst),
            4,
            "the start's probe and three retries; the start's last look reads the cached timeout"
        );
    }

    /// Waits up to `limit` for `done`, polling.
    fn within(limit: Duration, done: impl Fn() -> bool) -> bool {
        let deadline = Instant::now() + limit;
        while !done() {
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(10));
        }
        true
    }

    #[test]
    fn a_gated_chat_read_does_not_wait_for_a_pending_probe() {
        let (agents, _, profile) = service();
        let (runner, runs) = scripted(&[Step::Hang]);
        // A long wait: the retry stays pending for the whole test.
        fake_chat_kind(&agents, profile.path(), runner, &[Duration::from_secs(30)]);
        // The read before any gate: the probe times out, and a retry is pending.
        listed_chat(&agents);
        assert_eq!(agents.check_retry_threads(), 1, "a retry is pending");
        let (decl, program) = agents.runnable_kind("fakechat").expect("the kind runs");
        let gate = Arc::new(std::sync::Mutex::new(()));
        let (entered, has_entered) = std::sync::mpsc::channel();

        let held = {
            let (gate, agents) = (Arc::clone(&gate), agents.clone());
            thread::spawn(move || {
                let _gate = gate.lock().unwrap();
                entered.send(()).unwrap();
                agents.chat_adapter_of(&decl, &program).map(drop)
            })
        };
        has_entered.recv().unwrap();

        assert!(
            within(Duration::from_secs(3), || gate.try_lock().is_ok()),
            "another gated operation waited for the pending probe"
        );
        let refused = held
            .join()
            .unwrap()
            .expect_err("a timed-out probe is no support");
        assert_eq!(
            refused.code,
            massorca_protocol::codes::AGENT_CHAT_UNSUPPORTED
        );
        assert_eq!(
            runs.load(Ordering::SeqCst),
            1,
            "the gated read ran no probe: the timeout is cached"
        );
    }

    #[test]
    fn an_explicit_check_during_a_running_retry_keeps_the_retrys_verdict() {
        let (agents, _, profile) = service();
        let (running, is_running) = std::sync::mpsc::channel();
        let (release, released) = std::sync::mpsc::channel::<()>();
        let (running, released) = (
            std::sync::Mutex::new(running),
            std::sync::Mutex::new(released),
        );
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&runs);
        // The import signs in, an explicit check times out, and its retry holds until released, then signs in.
        let runner: CheckRunner =
            Arc::new(
                move |_program, _args| match counter.fetch_add(1, Ordering::SeqCst) {
                    0 => run_of(Step::SignedIn),
                    1 => run_of(Step::Hang),
                    _ => {
                        running.lock().unwrap().send(()).ok();
                        released
                            .lock()
                            .unwrap()
                            .recv_timeout(Duration::from_secs(30))
                            .ok();
                        run_of(Step::SignedIn)
                    }
                },
            );
        fake_kind(&agents, profile.path(), runner, &QUICK);
        let source = source_with_token();
        let account = agents
            .accounts_import("fake", source.path().to_str(), None)
            .expect("the import runs")
            .account
            .expect("kept");
        // The account is undecided in the store, as a sign-in interrupted by a restart leaves it.
        agents
            .store()
            .set_agent_account_pending(&account.id, true)
            .expect("marked undecided");
        agents
            .accounts_check("fake", Some(&account.id))
            .expect("the check runs and times out");
        is_running
            .recv_timeout(Duration::from_secs(10))
            .expect("the retry runs");

        let busy = agents
            .accounts_check("fake", Some(&account.id))
            .expect_err("the retry holds the account");

        assert_eq!(busy.code, massorca_protocol::codes::AGENT_BUSY);
        release.send(()).unwrap();
        retries_settle(&agents);
        assert!(
            !agents
                .store_pending(&account.id)
                .expect("the store answers"),
            "the retry's verdict decided the account"
        );
        assert_eq!(runs.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn a_panicking_retry_leaves_no_entry() {
        let (agents, _, profile) = service();
        let (runner, _runs) = scripted(&[Step::Hang, Step::Panic]);
        fake_chat_kind(&agents, profile.path(), runner, &QUICK);

        listed_chat(&agents);

        assert!(
            within(Duration::from_secs(10), || agents.check_retry_threads()
                == 0),
            "the panicked retry's thread is still counted"
        );
        assert!(
            agents.check_retries().current.is_empty(),
            "the panicked retry's entry is gone"
        );
    }
}
