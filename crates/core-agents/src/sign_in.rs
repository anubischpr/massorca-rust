//! Sign-in to an account (decision 0059): the account's empty home, a service-owned terminal that runs the kind's login
//! with the home variable set, and the decision when that terminal ends: keep the account with what its check found, or
//! delete it. The startup sweep makes the same decision for a sign-in an earlier service left open, except that a check
//! that cannot tell leaves that sign-in pending until a check decides it.

use std::thread;
use std::time::{Duration, Instant};

use massorca_core_store::{AgentAccountRow, new_id, now_ms};
use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::{
    CreateSpec, ShellChoice, TerminalManager, TerminalState as CoreState,
};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{AccountState, AccountsAdded};
use massorca_protocol::methods::terminals::{DEFAULT_COLS, DEFAULT_ROWS};

use crate::account_check::{Verdict, account_strip_names, sign_in_env, state_name};
use crate::accounts::{
    DEFAULT_LABEL, DecisionMark, ORIGIN_IMPORT, ORIGIN_SIGN_IN, refused_state, valid_label,
};
use crate::credentials;
use crate::sessions::{AgentError, Agents};

/// How often a sign-in terminal's exit is looked for.
const SIGN_IN_POLL: Duration = Duration::from_millis(200);
/// The scope a sign-in terminal is listed under: no workspace owns it.
pub const SIGN_IN_SCOPE: &str = "agent-accounts";
/// How long a sign-in terminal may stay open with no result before it is closed and its account decided (decision 0059).
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// A test hook that shortens [`SIGN_IN_TIMEOUT`] for the service's tests: whole milliseconds, clamped to 100 ms through the
/// default, read when the service starts. A value that is not a whole number is ignored.
pub const SIGN_IN_TIMEOUT_ENV: &str = "MASSORCA_SIGN_IN_TIMEOUT_MS";
/// The shortest time limit the test hook can set: a shorter one would close a sign-in before its login has started.
const SIGN_IN_TIMEOUT_FLOOR: Duration = Duration::from_millis(100);

impl Agents {
    /// Starts a sign-in (decision 0059): an empty account home, and a service-owned terminal that runs the kind's login
    /// with the home variable set. The account is pending until the terminal ends; a pending account cannot be removed or
    /// activated. When the terminal ends, see [`Agents::decide_sign_in`].
    pub fn accounts_add(
        &self,
        kind: &str,
        label: Option<String>,
    ) -> Result<AccountsAdded, AgentError> {
        let label = label.unwrap_or_else(|| DEFAULT_LABEL.to_owned());
        valid_label(&label)?;
        let (decl, program) = self.startable_kind(kind)?;
        let credential_decl = decl
            .credentials
            .clone()
            .filter(|_| decl.accounts_supported())
            .ok_or_else(|| unsupported(kind))?;
        let login = decl
            .accounts
            .as_ref()
            .map(|accounts| accounts.login.clone())
            .unwrap_or_default();
        let account = new_id();
        let row = AgentAccountRow {
            id: account.clone(),
            kind: kind.to_owned(),
            label,
            created_at: now_ms(),
            identity: None,
            state: state_name(AccountState::NeedsSignIn).to_owned(),
            origin: ORIGIN_SIGN_IN.to_owned(),
        };
        let home = {
            let gate = credentials::lock_homes();
            self.migrate_legacy(&gate, kind)?;
            let home = credentials::create_account_home(self.profile_dir(), kind, &account)?;
            if let Err(error) = self.store().insert_pending_sign_in(&row) {
                let _ = std::fs::remove_dir_all(&home);
                return Err(error.into());
            }
            // Pending from the row's creation, under the same gate hold, so no removal can see the row without it.
            self.account_busy()
                .pending
                .insert(account.clone(), String::new());
            home
        };
        let env = sign_in_env(
            &credential_decl.home_env,
            &home,
            &account_strip_names(&decl),
        );
        let spec = CreateSpec {
            workspace_id: SIGN_IN_SCOPE.to_owned(),
            cwd: home,
            shell: Some(ShellChoice {
                program: program.to_string_lossy().into_owned(),
                args: login,
            }),
            env,
            size: PtySize {
                cols: DEFAULT_COLS,
                rows: DEFAULT_ROWS,
            },
            title: Some(format!("{} sign-in", decl.name)),
            request_id: None,
        };
        let terminal = match self.terminals().create(spec) {
            Ok(terminal) => terminal,
            Err(error) => {
                self.account_busy().pending.remove(&account);
                self.discard_account(kind, &account);
                return Err(error.into());
            }
        };
        self.account_busy()
            .pending
            .insert(account.clone(), terminal.id.clone());
        let summary = self.summary(
            &row,
            self.store().get_active_agent_account(kind)?.as_deref(),
        )?;
        self.notify_account(kind, &account, AccountState::NeedsSignIn);
        let agents = self.clone();
        let (kind_owned, watched, terminal_id) =
            (kind.to_owned(), account.clone(), terminal.id.clone());
        thread::spawn(move || agents.finish_sign_in(&kind_owned, &watched, &terminal_id));
        Ok(AccountsAdded {
            account: summary,
            terminal_id: terminal.id,
        })
    }

    /// Runs when a sign-in terminal ends. A terminal still open at its time limit is closed first, and then decided as an
    /// ended sign-in is (decision 0059): the account is no longer pending, and is decided.
    fn finish_sign_in(&self, kind: &str, account: &str, terminal: &str) {
        let deadline = Instant::now() + self.sign_in_timeout();
        if !wait_for_exit(self.terminals(), terminal, Some(deadline)) {
            tracing::warn!(kind, "a sign-in left open past its time limit is closed");
            // The kill returns once the exit is recorded, or its grace and margin have passed; the wait after it covers
            // a late record.
            let _ = self.terminals().kill(terminal, false);
            wait_for_exit(self.terminals(), terminal, None);
        }
        // The open mark becomes the decision's, so no other call acts on the account between the two (decision 0059).
        let mark = self.mark_now(account);
        self.decide_marked(kind, account, SignInEnd::TerminalEnded, mark);
    }

    /// Decides an undecided sign-in or import this service made (decision 0059), from its check, its declared files and what
    /// ended it ([`SignInEnd`], [`sign_in_outcome`]). A check that reports signed in keeps the account, decided, with the
    /// check's identity. A check that cannot tell keeps the account as `unknown`, decided, while its files exist, when its
    /// terminal ended; the startup sweep keeps a sign-in pending instead, so the next start decides it again. Any other
    /// verdict, or no declared file left, signs a sign-in out and deletes it. A row that is already gone is left alone.
    pub(crate) fn decide_sign_in(&self, kind: &str, account: &str, end: SignInEnd) {
        // A call that holds the account decides it, or is busy with it: either way this decision is not needed here.
        let Ok(mark) = self.mark_for_decision(account) else {
            return;
        };
        self.decide_marked(kind, account, end, mark);
    }

    /// Decides an account whose mark the caller holds. Only an undecided sign-in or import of this kind is decided, so no
    /// other row is deleted from here (decision 0059). The outcome is stored before the mark is dropped.
    fn decide_marked(&self, kind: &str, account: &str, end: SignInEnd, mark: DecisionMark) {
        let origin = match self.store().get_agent_account(account) {
            Ok(Some(row)) if row.kind == kind => row.origin,
            _ => return,
        };
        let undecided = self.store_pending(account).unwrap_or(false);
        if !undecided || (origin != ORIGIN_SIGN_IN && origin != ORIGIN_IMPORT) {
            return;
        }
        let files =
            credentials::account_files_present(self.profile_dir(), kind, account).unwrap_or(false);
        let kind_program = self.startable_kind(kind).ok();
        let verdict = kind_program
            .as_ref()
            .and_then(|(decl, program)| self.check_account_home(decl, program, account, None));
        let outcome = sign_in_outcome(verdict.as_ref(), files, end);
        let refused = refused_state(verdict.as_ref());
        // A refused sign-in is signed out first, as a removal signs out (decision 0059); an import never is.
        let sign_out = if origin == ORIGIN_SIGN_IN {
            kind_program
        } else {
            None
        };
        self.store_outcome(kind, account, outcome, mark, sign_out, refused);
    }
}

/// Waits until the terminal has exited, or can no longer be found (it was closed). With a `deadline`, gives up at it and
/// returns `false` while the terminal still runs.
fn wait_for_exit(terminals: &TerminalManager, terminal: &str, deadline: Option<Instant>) -> bool {
    loop {
        match terminals.get(terminal) {
            Ok(info) if !matches!(info.state, CoreState::Exited) => {
                if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    return false;
                }
                thread::sleep(SIGN_IN_POLL);
            }
            _ => return true,
        }
    }
}

fn unsupported(kind: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_CREDENTIALS_UNSUPPORTED,
        format!(
            "agent kind {kind} has no sign-in in MassOrca: sign in from the vendor's CLI, then import the login"
        ),
    )
}

/// When a sign-in is decided: its terminal has ended (or was closed at its time limit), or the startup sweep finds it open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SignInEnd {
    /// The terminal ended, normally or at its time limit: a check that cannot tell keeps the account, as `unknown`, decided.
    TerminalEnded,
    /// The startup sweep decides a sign-in an earlier service left open: a check that cannot tell keeps the account
    /// pending, so the next start decides it again.
    StartupSweep,
}

/// What a sign-in account becomes once it is decided (decision 0059).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SignInOutcome {
    /// The check reports signed in: the account is kept and decided, with the check's identity.
    SignedIn { identity: Option<String> },
    /// The check cannot tell and the declared files exist, when the terminal ended: kept and decided, as `unknown`.
    Unknown,
    /// The check cannot tell and the declared files exist, at startup: kept as `unknown`, and still pending.
    Undecided,
    /// The account is not kept: a check reported signed out, or no declared file is left.
    Discard,
}

/// The sign-in time limit for a value of [`SIGN_IN_TIMEOUT_ENV`]: whole milliseconds, clamped to the hook's range; any other
/// value, or none, is the default.
pub(crate) fn sign_in_timeout_from(value: Option<&str>) -> Duration {
    value
        .and_then(|text| text.trim().parse::<u64>().ok())
        .map_or(SIGN_IN_TIMEOUT, |millis| {
            Duration::from_millis(millis).clamp(SIGN_IN_TIMEOUT_FLOOR, SIGN_IN_TIMEOUT)
        })
}

/// Decides a sign-in account from its check's verdict, whether its declared files are present, and what ended the sign-in
/// (decision 0059). A verdict other than `unknown` has judged the login: `signed_in` keeps it, any other state discards it.
/// Without a verdict, or with `unknown`, the check cannot tell: the login's files keep the account as `unknown`, and without
/// them nothing is kept. The end of the terminal decides it; only the startup sweep leaves it pending.
pub(crate) fn sign_in_outcome(
    verdict: Option<&Verdict>,
    files_present: bool,
    end: SignInEnd,
) -> SignInOutcome {
    match verdict {
        Some(found) if found.state == AccountState::SignedIn => SignInOutcome::SignedIn {
            identity: found.identity.clone(),
        },
        Some(found) if found.state != AccountState::Unknown => SignInOutcome::Discard,
        _ if !files_present => SignInOutcome::Discard,
        _ => match end {
            SignInEnd::TerminalEnded => SignInOutcome::Unknown,
            SignInEnd::StartupSweep => SignInOutcome::Undecided,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(state: AccountState, identity: Option<&str>) -> Verdict {
        Verdict {
            state,
            identity: identity.map(str::to_owned),
        }
    }

    #[test]
    fn the_sign_in_time_limit_is_fifteen_minutes_unless_the_test_hook_sets_it() {
        assert_eq!(SIGN_IN_TIMEOUT, Duration::from_secs(15 * 60));
        assert_eq!(sign_in_timeout_from(None), SIGN_IN_TIMEOUT);
        assert_eq!(
            sign_in_timeout_from(Some("soon")),
            SIGN_IN_TIMEOUT,
            "not a whole number"
        );
    }

    #[test]
    fn the_test_hook_sets_the_time_limit_clamped_between_a_tenth_of_a_second_and_the_default() {
        assert_eq!(
            sign_in_timeout_from(Some("250")),
            Duration::from_millis(250)
        );
        assert_eq!(
            sign_in_timeout_from(Some(" 400 ")),
            Duration::from_millis(400)
        );
        assert_eq!(
            sign_in_timeout_from(Some("100")),
            Duration::from_millis(100)
        );
        assert_eq!(
            sign_in_timeout_from(Some("40")),
            Duration::from_millis(100),
            "clamped up to the floor"
        );
        assert_eq!(
            sign_in_timeout_from(Some("0")),
            Duration::from_millis(100),
            "clamped up to the floor"
        );
        assert_eq!(sign_in_timeout_from(Some("900000")), SIGN_IN_TIMEOUT);
        assert_eq!(
            sign_in_timeout_from(Some("99999999999")),
            SIGN_IN_TIMEOUT,
            "clamped down to the default"
        );
    }

    #[test]
    fn a_check_that_reports_signed_in_keeps_the_account_with_its_identity() {
        for end in [SignInEnd::TerminalEnded, SignInEnd::StartupSweep] {
            assert_eq!(
                sign_in_outcome(
                    Some(&found(AccountState::SignedIn, Some("fake@example.com"))),
                    true,
                    end
                ),
                SignInOutcome::SignedIn {
                    identity: Some("fake@example.com".to_owned())
                }
            );
            assert_eq!(
                sign_in_outcome(Some(&found(AccountState::SignedIn, None)), false, end),
                SignInOutcome::SignedIn { identity: None }
            );
        }
    }

    #[test]
    fn a_check_that_reports_signed_out_discards_the_account_even_with_its_files() {
        for end in [SignInEnd::TerminalEnded, SignInEnd::StartupSweep] {
            for state in [
                AccountState::NeedsSignIn,
                AccountState::Expired,
                AccountState::Conflict,
            ] {
                assert_eq!(
                    sign_in_outcome(Some(&found(state, None)), true, end),
                    SignInOutcome::Discard,
                    "{state:?} at {end:?}"
                );
            }
        }
    }

    #[test]
    fn a_check_that_cannot_tell_keeps_the_account_as_unknown_when_its_terminal_ended() {
        assert_eq!(
            sign_in_outcome(
                Some(&found(AccountState::Unknown, None)),
                true,
                SignInEnd::TerminalEnded
            ),
            SignInOutcome::Unknown
        );
        assert_eq!(
            sign_in_outcome(None, true, SignInEnd::TerminalEnded),
            SignInOutcome::Unknown,
            "a kind without a check, or one that could not run"
        );
    }

    #[test]
    fn a_check_that_cannot_tell_leaves_the_account_pending_at_startup() {
        assert_eq!(
            sign_in_outcome(
                Some(&found(AccountState::Unknown, None)),
                true,
                SignInEnd::StartupSweep
            ),
            SignInOutcome::Undecided
        );
        assert_eq!(
            sign_in_outcome(None, true, SignInEnd::StartupSweep),
            SignInOutcome::Undecided
        );
    }

    #[test]
    fn a_sign_in_with_no_declared_file_left_is_discarded_whatever_the_check_cannot_tell() {
        for end in [SignInEnd::TerminalEnded, SignInEnd::StartupSweep] {
            assert_eq!(
                sign_in_outcome(Some(&found(AccountState::Unknown, None)), false, end),
                SignInOutcome::Discard
            );
            assert_eq!(sign_in_outcome(None, false, end), SignInOutcome::Discard);
        }
    }
}
