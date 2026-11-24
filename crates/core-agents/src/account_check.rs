//! What an account check finds (decision 0059): the state a check's exit code and output imply, the identity it captures,
//! the conflict with a recorded identity, and the bounded run of the kind's program that produces that output. The
//! output is matched here and never logged or returned.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use massorca_protocol::methods::agents::AccountState;

use crate::kind::{CheckDecl, KindDecl};
use crate::process_tree;

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
/// `conflict`; the returned identity is then `None`, so the store keeps the recorded one. Identities compare trimmed and
/// without case.
pub fn settle(found: Verdict, recorded: Option<&str>) -> Verdict {
    match (found.state, found.identity.as_deref(), recorded) {
        (AccountState::SignedIn, Some(identity), Some(known))
            if normalized_identity(identity) != normalized_identity(known) =>
        {
            Verdict {
                state: AccountState::Conflict,
                identity: None,
            }
        }
        _ => found,
    }
}

/// An identity as two accounts are compared by it: trimmed, without case (ACC-03).
pub(crate) fn normalized_identity(identity: &str) -> String {
    identity.trim().to_lowercase()
}

fn pattern_matches(pattern: Option<&str>, output: &str) -> bool {
    pattern
        .and_then(|pattern| regex::Regex::new(pattern).ok())
        .is_some_and(|regex| regex.is_match(output))
}

/// The most characters an account's identity or label may hold (decision 0059).
pub const MAX_TEXT_CHARS: usize = 256;

/// Whether a captured identity may be stored: not empty, at most [`MAX_TEXT_CHARS`] characters, and no control
/// character. A capture that fails this stores no identity at all; the check's state still stands.
fn valid_identity(identity: &str) -> bool {
    !identity.is_empty()
        && identity.chars().count() <= MAX_TEXT_CHARS
        && !identity.chars().any(char::is_control)
}

fn capture(pattern: Option<&str>, output: &str) -> Option<String> {
    let regex = regex::Regex::new(pattern?).ok()?;
    let captures = regex.captures(output)?;
    captures
        .get(1)
        .or_else(|| captures.get(0))
        .map(|found| found.as_str().trim().to_owned())
        .filter(|identity| valid_identity(identity))
}

/// What a run of a kind's program produced: its exit code (`None` when it did not finish) and its output, which is
/// matched and never logged.
pub(crate) struct RunOutput {
    pub(crate) exit: Option<i32>,
    pub(crate) output: String,
}

/// How long output is waited for after the program's tree is stopped. A stopped tree closes its pipes, so this bounds
/// only a pipe something outside the tree still holds.
const OUTPUT_GRACE: Duration = Duration::from_secs(1);

/// The variables the account commands (check, sign-in, sign-out) remove from their environment: the credentials'
/// `strip_env`, then the accounts' `strip_env`, each name once. Agent runs keep their environment (decision 0059, amended
/// 2026-11-22).
pub(crate) fn account_strip_names(decl: &KindDecl) -> Vec<String> {
    let credentials = decl
        .credentials
        .iter()
        .flat_map(|credentials| credentials.strip_env.iter());
    let accounts = decl
        .accounts
        .iter()
        .flat_map(|accounts| accounts.strip_env.iter());
    let mut names: Vec<String> = Vec::new();
    for name in credentials.chain(accounts) {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    names
}

/// The command of a check or a sign-out: `program` with `args`, the account's home variable set, and `strip` removed
/// from its environment. A plain process, so the names are removed for real.
pub(crate) fn account_command(
    program: &Path,
    args: &[String],
    home: &Path,
    home_env: &str,
    strip: &[String],
) -> Command {
    let mut command = massorca_process::command(program);
    command
        .args(args)
        .env(home_env, home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for name in strip {
        command.env_remove(name);
    }
    command
}

/// The environment of the sign-in terminal: the home variable, and each stripped name set to empty, because the terminal
/// can only add variables (docs/architecture/agents.md).
pub(crate) fn sign_in_env(
    home_env: &str,
    home: &Path,
    strip: &[String],
) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    env.insert(home_env.to_owned(), home.display().to_string());
    for name in strip {
        env.entry(name.clone()).or_default();
    }
    env
}

/// Runs `program` with `args`, the account's home variable set and the stripped names removed, for at most
/// [`CHECK_TIMEOUT`]. A program that cannot start, or is still running at the deadline, has no exit code. Its whole
/// process tree is stopped before this returns, so nothing it started keeps the output open (decision 0059).
pub(crate) fn run_program(
    program: &Path,
    args: &[String],
    home: &Path,
    home_env: &str,
    strip_env: &[String],
) -> RunOutput {
    let mut command = account_command(program, args, home, home_env, strip_env);
    let Ok((mut child, mut tree)) = process_tree::spawn(&mut command) else {
        return RunOutput {
            exit: None,
            output: String::new(),
        };
    };
    let stdout = read_limited(child.stdout.take());
    let stderr = read_limited(child.stderr.take());
    let exit = wait_with_deadline(&mut child);
    tree.stop();
    let mut output = collect(stdout);
    output.push_str(&collect(stderr));
    RunOutput { exit, output }
}

/// Reads a stream to its end, or to the output limit, on a thread of its own. The text arrives on the channel once read.
fn read_limited<R: Read + Send + 'static>(stream: Option<R>) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(stream) = stream {
            let _ = stream.take(CHECK_OUTPUT_LIMIT).read_to_end(&mut bytes);
        }
        let _ = sender.send(String::from_utf8_lossy(&bytes).into_owned());
    });
    receiver
}

/// The text of a stream, or nothing when it did not end within [`OUTPUT_GRACE`].
fn collect(stream: mpsc::Receiver<String>) -> String {
    stream.recv_timeout(OUTPUT_GRACE).unwrap_or_default()
}

/// The program's exit code, when it exits before the deadline; at the deadline it is killed and has none.
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

    /// The same login with other case or spaces is the same identity, not a conflict (ACC-03).
    #[test]
    fn the_same_identity_in_other_case_is_not_a_conflict() {
        let found = Verdict {
            state: AccountState::SignedIn,
            identity: Some(" Fake@Example.com".to_owned()),
        };
        assert_eq!(
            settle(found, Some("fake@example.com")).state,
            AccountState::SignedIn
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
    fn an_identity_over_256_characters_or_with_a_control_character_is_not_stored() {
        let pattern = Some(r"who (\S+)");
        let long = format!("who {}", "a".repeat(257));
        assert_eq!(capture(pattern, &long), None, "over the limit");
        let at_limit = format!("who {}", "a".repeat(256));
        assert_eq!(
            capture(pattern, &at_limit).map(|id| id.chars().count()),
            Some(256)
        );
        assert_eq!(
            capture(pattern, "who bad\u{7}name"),
            None,
            "a control character"
        );
    }

    #[test]
    fn an_identity_without_a_group_is_the_whole_match() {
        assert_eq!(
            capture(Some("fake@[a-z.]+"), "signed in: fake@example.com").as_deref(),
            Some("fake@example.com")
        );
    }

    #[test]
    fn the_three_account_commands_apply_both_strip_lists() {
        let decl = KindDecl::from_json(
            r#"{
                "id": "fake", "name": "Fake", "program": ["fake-agent"],
                "credentials": { "home_env": "FAKE_HOME", "files": ["token.json"], "strip_env": ["CRED_ONE"] },
                "accounts": {
                    "login": ["login"], "check": { "args": ["whoami"] }, "logout": ["logout"],
                    "strip_env": ["ACCOUNT_ONE", "ACCOUNT_TWO"]
                }
            }"#,
        )
        .expect("parses");
        let names = account_strip_names(&decl);
        assert_eq!(names, ["CRED_ONE", "ACCOUNT_ONE", "ACCOUNT_TWO"]);
        let home = Path::new("account-home");

        // The check and the sign-out are plain processes: both names are removed from their environment.
        let command = account_command(
            Path::new("fake-agent"),
            &["whoami".to_owned()],
            home,
            "FAKE_HOME",
            &names,
        );
        let removed: Vec<String> = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(key, _)| key.to_string_lossy().into_owned())
            .collect();
        for name in &names {
            assert!(
                removed.iter().any(|removed| removed == name),
                "{name} is removed"
            );
        }

        // The sign-in terminal can only add variables, so each name is set to empty there.
        let env = sign_in_env("FAKE_HOME", home, &names);
        for name in &names {
            assert_eq!(
                env.get(name).map(String::as_str),
                Some(""),
                "{name} is empty"
            );
        }
        assert_eq!(
            env.get("FAKE_HOME").map(String::as_str),
            Some(home.to_string_lossy().as_ref())
        );
    }
}
