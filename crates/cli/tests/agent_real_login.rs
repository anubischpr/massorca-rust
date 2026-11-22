//! Opt-in smoke test for a built-in kind's real sign-in (decision 0059, `docs/architecture/agents.md`, "Built-in
//! sign-in commands").
//!
//! It never runs in CI and never runs unless `MASSORCA_REAL_LOGIN_SMOKE` names a kind with a confirmed sign-in. It uses a
//! fresh MassOrca home, so the account home is new and empty and never the owner's default credential home. It starts the
//! kind's real login in a service-owned terminal, through `agent accounts add --no-attach`, and checks two things:
//! the account is listed as `needs_sign_in` while the terminal runs, and it is discarded when the terminal is closed
//! without signing in. The test never drives a login itself; it closes the terminal after a short hold.
//!
//! Run it from `crates/cli` or the workspace root, one kind at a time:
//!
//! ```text
//! MASSORCA_REAL_LOGIN_SMOKE=claude MASSORCA_REAL_LOGIN_HOLD_SECS=60 \
//!   cargo test -p massorca-cli --test agent_real_login -- --ignored --nocapture
//! ```
//!
//! `MASSORCA_REAL_LOGIN_HOLD_SECS` (default 30) is how long the terminal stays open, so a person can attach with
//! `massorca terminal attach <terminal id>` and look at it. The real CLI must be on the service's `PATH`.

mod harness;

use std::io::Write;
use std::thread;
use std::time::{Duration, Instant};

use harness::Sandbox;
use serde_json::Value;

const SMOKE_ENV: &str = "MASSORCA_REAL_LOGIN_SMOKE";
const HOLD_ENV: &str = "MASSORCA_REAL_LOGIN_HOLD_SECS";
const DEFAULT_HOLD_SECS: u64 = 30;
/// The kinds whose sign-in is confirmed from a primary source, and so have a smoke test.
const CONFIRMED_KINDS: &[&str] = &["claude"];
const STATE_TIMEOUT: Duration = Duration::from_secs(60);
const DISCARD_TIMEOUT: Duration = Duration::from_secs(120);
const POLL: Duration = Duration::from_millis(500);

#[test]
#[ignore = "opt-in: set MASSORCA_REAL_LOGIN_SMOKE to a confirmed kind (claude) and run with --ignored"]
fn a_real_sign_in_is_needs_sign_in_while_running_and_discarded_when_closed_unsigned() {
    let Ok(kind) = std::env::var(SMOKE_ENV) else {
        note(&format!(
            "skipped: set {SMOKE_ENV} to one of {CONFIRMED_KINDS:?}"
        ));
        return;
    };
    assert!(
        CONFIRMED_KINDS.contains(&kind.as_str()),
        "{kind} has no confirmed sign-in; the smoke test covers {CONFIRMED_KINDS:?}"
    );
    let hold = Duration::from_secs(
        std::env::var(HOLD_ENV)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(DEFAULT_HOLD_SECS),
    );

    // A fresh MassOrca home: the account home the service creates is new and empty.
    let sandbox = Sandbox::new();
    let added = sandbox.json_ok(
        "default",
        &[
            "agent",
            "accounts",
            "add",
            kind.as_str(),
            "--label",
            "smoke",
            "--no-attach",
        ],
    );
    let account = added["account"]["id"]
        .as_str()
        .expect("the sign-in names its account")
        .to_owned();
    let terminal = added["terminal_id"]
        .as_str()
        .expect("the sign-in names its terminal")
        .to_owned();

    // While the terminal runs, the account is pending: listed as needs_sign_in.
    let listed = wait_for_state(&sandbox, &kind, &account, "needs_sign_in");
    assert_eq!(listed["state"], "needs_sign_in", "{listed}");
    note(&format!(
        "sign-in terminal {terminal} is open for account {account}. Attach with `massorca terminal attach {terminal}` \
         to look at it. It is closed without signing in after {} seconds.",
        hold.as_secs()
    ));
    thread::sleep(hold);

    // Closed without signing in: the check at the end reports not signed in, so the account is discarded.
    sandbox.json_ok("default", &["terminal", "close", terminal.as_str()]);
    wait_until_discarded(&sandbox, &kind, &account);
}

/// The listed account with `account`, polled until its state is `state`.
fn wait_for_state(sandbox: &Sandbox, kind: &str, account: &str, state: &str) -> Value {
    let deadline = Instant::now() + STATE_TIMEOUT;
    loop {
        let found = accounts_of(sandbox, kind)
            .into_iter()
            .find(|row| row["id"] == account);
        if let Some(row) = &found
            && row["state"] == state
        {
            return row.clone();
        }
        assert!(
            Instant::now() < deadline,
            "the account never became {state}; last seen: {found:?}"
        );
        thread::sleep(POLL);
    }
}

/// Polled until the account is no longer listed. A check that does not finish keeps it as `unknown`, which fails here.
fn wait_until_discarded(sandbox: &Sandbox, kind: &str, account: &str) {
    let deadline = Instant::now() + DISCARD_TIMEOUT;
    loop {
        let found = accounts_of(sandbox, kind)
            .into_iter()
            .find(|row| row["id"] == account);
        match found {
            None => return,
            Some(row) => assert!(
                Instant::now() < deadline,
                "a sign-in closed without signing in must be discarded; still listed: {row}"
            ),
        }
        thread::sleep(POLL);
    }
}

fn accounts_of(sandbox: &Sandbox, kind: &str) -> Vec<Value> {
    sandbox.json_ok("default", &["agent", "accounts", "list", kind])["accounts"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// Writes a progress line straight to stderr, which libtest does not capture. `writeln!` rather than `eprintln!`: the
/// workspace denies `clippy::print_stderr`.
fn note(message: &str) {
    let _ = writeln!(std::io::stderr(), "{message}");
}
