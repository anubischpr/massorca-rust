//! What an account check finds (decision 0059): the state a check's exit code and output imply, the identity it captures,
//! the conflict with a recorded identity, and the bounded run of the kind's program that produces that output. The
//! output is matched here and never logged or returned.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use massorca_protocol::methods::agents::AccountState;

use crate::kind::CheckDecl;

/// How long an account check may run before it is stopped; a check that does not finish has state `unknown`.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(10);
/// The most output of a check that is kept. It is matched, never logged or returned.
const CHECK_OUTPUT_LIMIT: u64 = 64 * 1024;

/// The stored name of a state.
pub fn state_name(state: AccountState) -> &'static str {
    match state {
        AccountState::SignedIn => "signed_in",
        AccountState::Expired => "expired",
        AccountState::NeedsSignIn => "needs_sign_in",
        AccountState::NotSetUp => "not_set_up",
        AccountState::Migrated => "migrated",
        AccountState::Conflict => "conflict",
        AccountState::Unknown => "unknown",
    }
}

/// The state a stored name stands for; a name this build does not know reads as `unknown`.
pub fn parse_state(name: &str) -> AccountState {
    match name {
        "signed_in" => AccountState::SignedIn,
        "expired" => AccountState::Expired,
        "needs_sign_in" => AccountState::NeedsSignIn,
        "not_set_up" => AccountState::NotSetUp,
        "migrated" => AccountState::Migrated,
        "conflict" => AccountState::Conflict,
        _ => AccountState::Unknown,
    }
}

/// What a check found: the state it implies, and the identity it captured, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub state: AccountState,
    pub identity: Option<String>,
}

pub(crate) fn unknown() -> Verdict {
    Verdict {
        state: AccountState::Unknown,
        identity: None,
    }
}

/// Maps a check's exit code and output to a state (decision 0059). No exit means the check did not finish, so the state
/// is unknown. A match of the expired pattern means `expired`, whatever the exit code. The signed-in exit means signed in,
/// with the identity pattern's first group, or its whole match, as the identity. Any other exit needs a sign-in.
pub fn verdict(check: &CheckDecl, exit: Option<i32>, output: &str) -> Verdict {
    let Some(exit) = exit else {
        return unknown();
    };
    if pattern_matches(check.expired_pattern.as_deref(), output) {
        return Verdict {
            state: AccountState::Expired,
            identity: None,
        };
    }
    if exit == check.signed_in_exit {
        return Verdict {
            state: AccountState::SignedIn,
            identity: capture(check.identity_pattern.as_deref(), output),
        };
    }
    Verdict {
        state: AccountState::NeedsSignIn,
        identity: None,
    }
}

/// The state after a check, given the identity recorded for the account. A different identity than the recorded one is a
/// `conflict`; the returned identity is then `None`, so the store keeps the recorded one.
pub fn settle(found: Verdict, recorded: Option<&str>) -> Verdict {
    match (found.state, found.identity.as_deref(), recorded) {
        (AccountState::SignedIn, Some(identity), Some(known)) if identity != known => Verdict {
            state: AccountState::Conflict,
            identity: None,
        },
        _ => found,
    }
}

fn pattern_matches(pattern: Option<&str>, output: &str) -> bool {
    pattern
        .and_then(|pattern| regex::Regex::new(pattern).ok())
        .is_some_and(|regex| regex.is_match(output))
}

fn capture(pattern: Option<&str>, output: &str) -> Option<String> {
    let regex = regex::Regex::new(pattern?).ok()?;
    let captures = regex.captures(output)?;
    captures
        .get(1)
        .or_else(|| captures.get(0))
        .map(|found| found.as_str().trim().to_owned())
        .filter(|identity| !identity.is_empty())
}

/// What a run of a kind's program produced: its exit code (`None` when it did not finish) and its output, which is
/// matched and never logged.
pub(crate) struct RunOutput {
    pub(crate) exit: Option<i32>,
    pub(crate) output: String,
}

/// Runs `program` with `args`, the account's home variable set and the stripped names removed, for at most
/// [`CHECK_TIMEOUT`]. A program that cannot start, or is still running at the deadline, has no exit code.
pub(crate) fn run_program(
    program: &Path,
    args: &[String],
    home: &Path,
    home_env: &str,
    strip_env: &[String],
) -> RunOutput {
    let mut command = Command::new(program);
    command
        .args(args)
        .env(home_env, home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for name in strip_env {
        command.env_remove(name);
    }
    let Ok(mut child) = command.spawn() else {
        return RunOutput {
            exit: None,
            output: String::new(),
        };
    };
    let stdout = read_limited(child.stdout.take());
    let stderr = read_limited(child.stderr.take());
    let exit = wait_with_deadline(&mut child);
    let mut output = stdout.join().unwrap_or_default();
    output.push_str(&stderr.join().unwrap_or_default());
    RunOutput { exit, output }
}

fn read_limited<R: Read + Send + 'static>(stream: Option<R>) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(stream) = stream {
            let _ = stream.take(CHECK_OUTPUT_LIMIT).read_to_end(&mut bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

fn wait_with_deadline(child: &mut Child) -> Option<i32> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.code(),
            Ok(None) if started.elapsed() < CHECK_TIMEOUT => {
                thread::sleep(Duration::from_millis(25));
            }
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

    fn check(expired: Option<&str>, identity: Option<&str>) -> CheckDecl {
        CheckDecl {
            args: vec!["whoami".to_owned()],
            signed_in_exit: 0,
            expired_pattern: expired.map(str::to_owned),
            identity_pattern: identity.map(str::to_owned),
        }
    }

    #[test]
    fn the_stored_names_round_trip_and_an_unknown_name_reads_as_unknown() {
        for state in [
            AccountState::SignedIn,
            AccountState::Expired,
            AccountState::NeedsSignIn,
            AccountState::NotSetUp,
            AccountState::Migrated,
            AccountState::Conflict,
            AccountState::Unknown,
        ] {
            assert_eq!(parse_state(state_name(state)), state);
        }
        assert_eq!(parse_state("refreshing"), AccountState::Unknown);
    }

    #[test]
    fn the_signed_in_exit_with_an_identity_is_signed_in() {
        let found = verdict(
            &check(None, Some(r"signed in as (\S+)")),
            Some(0),
            "signed in as fake@example.com\n",
        );
        assert_eq!(found.state, AccountState::SignedIn);
        assert_eq!(found.identity.as_deref(), Some("fake@example.com"));
    }

    #[test]
    fn an_expired_match_wins_over_the_exit_code() {
        let found = verdict(&check(Some("expired"), None), Some(0), "expired\n");
        assert_eq!(found.state, AccountState::Expired);
        assert_eq!(found.identity, None);
    }

    #[test]
    fn another_exit_needs_a_sign_in_and_no_exit_is_unknown() {
        assert_eq!(
            verdict(&check(None, None), Some(1), "").state,
            AccountState::NeedsSignIn
        );
        assert_eq!(
            verdict(&check(Some("expired"), None), None, "expired").state,
            AccountState::Unknown
        );
    }

    #[test]
    fn a_different_identity_is_a_conflict_and_the_recorded_one_stays() {
        let settled = settle(
            Verdict {
                state: AccountState::SignedIn,
                identity: Some("other@example.com".to_owned()),
            },
            Some("fake@example.com"),
        );
        assert_eq!(settled.state, AccountState::Conflict);
        assert_eq!(settled.identity, None);
        let same = settle(
            Verdict {
                state: AccountState::SignedIn,
                identity: Some("fake@example.com".to_owned()),
            },
            Some("fake@example.com"),
        );
        assert_eq!(same.state, AccountState::SignedIn);
    }

    #[test]
    fn an_identity_without_a_group_is_the_whole_match() {
        assert_eq!(
            capture(Some("fake@[a-z.]+"), "signed in: fake@example.com").as_deref(),
            Some("fake@example.com")
        );
    }
}
