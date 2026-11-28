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

