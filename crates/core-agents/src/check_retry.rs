//! Background retries of an account check stopped at its time limit (decision 0059, amended 2026-11-27). A check killed at
//! the cap is not a verdict: the account keeps its last known state, and the check is retried after each wait of
//! [`CHECK_RETRY_DELAYS`], one retry at a time per account, until one run decides. A retry applies its result through
//! the same path as `agent.accounts.check`, which announces a changed state with `agent.accounts_changed`. A decided
//! verdict is never retried. A newer explicit check supersedes a pending retry, and a removal cancels it.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::account_check::{CHECK_TIMEOUT, CheckRun, run_check};
use crate::sessions::{Agents, WeakAgents};

/// The waits before each retry of a timed-out check: three retries, the last about 22 seconds after the first check.
pub const CHECK_RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(15),
];

/// How often a waiting retry looks whether it was cancelled or superseded.
const RETRY_POLL: Duration = Duration::from_millis(25);

/// A stand-in for the check's program, given the check's arguments: tests use it to make a run finish or time out on cue.
pub(crate) type CheckRunner = Arc<dyn Fn(&[String]) -> CheckRun + Send + Sync>;

/// How account checks run.
#[derive(Clone)]
pub(crate) struct CheckSettings {
    /// How long a check may run before it is stopped.
    pub(crate) timeout: Duration,
    /// The waits before each retry of a timed-out check; empty retries nothing.
    pub(crate) retry_delays: Vec<Duration>,
    /// Runs instead of the check's program, when set.
    pub(crate) runner: Option<CheckRunner>,
}

impl Default for CheckSettings {
    fn default() -> Self {
        Self {
            timeout: CHECK_TIMEOUT,
            retry_delays: CHECK_RETRY_DELAYS.to_vec(),
            runner: None,
        }
    }
}

/// The retries waiting or running: the generation of each account's current retry, and how many retry threads exist.
#[derive(Debug, Default)]
pub(crate) struct CheckRetries {
    current: HashMap<String, u64>,
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
            return runner(args);
        }
        run_check(program, args, home, home_env, strip_env, settings.timeout)
    }

    /// Retries the check of `account` in the background, after a check of it timed out. A retry already waiting for the
    /// account is superseded by this one, so at most one is in flight.
    pub(crate) fn schedule_check_retry(&self, kind: &str, account: &str) {
        let delays = self.check_settings().retry_delays;
        if delays.is_empty() {
            return;
        }
        let generation = {
            let mut retries = self.check_retries();
            retries.next += 1;
            let generation = retries.next;
            retries.current.insert(account.to_owned(), generation);
            retries.threads += 1;
            generation
        };
        let weak = self.downgrade();
        let (kind, account) = (kind.to_owned(), account.to_owned());
        let spawned = thread::Builder::new()
            .name("account-check-retry".to_owned())
            .spawn({
                let weak = weak.clone();
                let account = account.clone();
                move || retry_loop(&weak, &kind, &account, generation, &delays)
            });
        if spawned.is_err() {
            tracing::warn!("an account check retry could not start");
            if let Some(agents) = weak.upgrade() {
                agents.end_retry(&account, generation);
            }
        }
    }

    /// Cancels a retry waiting for `account`, if any: the account was removed, or is being checked again.
    pub(crate) fn cancel_check_retry(&self, account: &str) {
        self.check_retries().current.remove(account);
    }

    /// Whether `generation` is still the retry wanted for `account`.
    pub(crate) fn retry_is_current(&self, account: &str, generation: u64) -> bool {
        self.check_retries().current.get(account) == Some(&generation)
    }

    /// How many retry threads exist, waiting or running.
    #[cfg(test)]
    pub(crate) fn check_retry_threads(&self) -> usize {
        self.check_retries().threads
    }

    /// Ends a retry thread: its account's entry goes when it is still this retry's.
    fn end_retry(&self, account: &str, generation: u64) {
        let mut retries = self.check_retries();
        if retries.current.get(account) == Some(&generation) {
            retries.current.remove(account);
        }
        retries.threads = retries.threads.saturating_sub(1);
    }
}

/// The body of a retry thread: after each wait, one check of the account, until a run decides, the retries run out, or
/// the retry is cancelled or superseded. A refused check (the account is gone, busy, or being removed) ends it.
fn retry_loop(weak: &WeakAgents, kind: &str, account: &str, generation: u64, delays: &[Duration]) {
    for delay in delays {
        if !wait_while_current(weak, account, generation, *delay) {
            break;
        }
        let Some(agents) = weak.upgrade() else {
            return;
        };
        match agents.check_for_retry(kind, account, generation) {
            Ok(true) => {
                tracing::info!(
                    kind,
                    "an account check timed out again; it is retried later"
                );
            }
            Ok(false) => break,
            Err(error) => {
                tracing::info!(kind, code = error.code, "an account check retry ended");
                break;
            }
        }
    }
    if let Some(agents) = weak.upgrade() {
        agents.end_retry(account, generation);
    }
}

/// Waits `delay`, and says whether the retry is still wanted at its end. Returns early once it is not.
fn wait_while_current(weak: &WeakAgents, account: &str, generation: u64, delay: Duration) -> bool {
    let due = Instant::now() + delay;
    loop {
        match weak.upgrade() {
            Some(agents) if agents.retry_is_current(account, generation) => {}
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
    }

    /// A stand-in check that takes `steps` in order, then repeats the last, and counts its runs.
    fn scripted(steps: &[Step]) -> (CheckRunner, Arc<AtomicUsize>) {
        let steps = steps.to_vec();
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&runs);
        let runner: CheckRunner = Arc::new(move |_args| {
            let index = counter.fetch_add(1, Ordering::SeqCst);
            let step = steps[index.min(steps.len() - 1)];
            let (exit, output, timed_out) = match step {
                Step::Hang => (None, "", true),
                Step::SignedIn => (
                    Some(0),
                    r#"{"loggedIn":true,"email":"fake@example.test"}"#,
                    false,
                ),
                Step::SignedOut => (Some(1), r#"{"loggedIn":false}"#, false),
            };
            CheckRun {
                run: RunOutput {
                    exit,
                    output: output.to_owned(),
                },
                timed_out,
            }
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
}
