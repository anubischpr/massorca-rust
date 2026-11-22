//! `massorca agent hook <event> [<payload>]`: the one entry point an agent's hooks call (decision 0053).
//!
//! It fails open: whatever happens it exits 0 within its budget and prints nothing on success, so a status problem
//! never blocks or breaks the agent. Problems go to stderr. The payload and the token are never printed.

use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use massorca_profile::Profile;
use massorca_protocol::methods::agents::{AgentHookParams, MAX_HOOK_PAYLOAD_BYTES};
use tokio::runtime::Builder;

use crate::connect::connect_running;

/// The session the run belongs to, set by the service in the run's environment.
pub const ENV_SESSION: &str = "MASSORCA_AGENT_SESSION";
/// The run's hook token, set by the service in the run's environment.
pub const ENV_TOKEN: &str = "MASSORCA_AGENT_TOKEN";
/// The whole command's budget, reading stdin included.
pub const BUDGET: Duration = Duration::from_secs(5);

/// Sends one report. Never fails: a problem is written to stderr and the command still exits 0. A missing event is
/// such a problem.
pub fn run(
    home: Option<PathBuf>,
    profile: Option<String>,
    event: Option<String>,
    payload: Option<String>,
) {
    let outcome = match event {
        Some(event) => report(home, profile, event, payload),
        None => Err("no event was given".to_owned()),
    };
    if let Err(problem) = outcome {
        let _ = writeln!(
            io::stderr().lock(),
            "massorca agent hook: {problem}; nothing was reported"
        );
    }
}

fn report(
    home: Option<PathBuf>,
    profile: Option<String>,
    event: String,
    payload: Option<String>,
) -> Result<(), String> {
    let deadline = Instant::now() + BUDGET;
    let session = env_value(ENV_SESSION)
        .ok_or_else(|| format!("{ENV_SESSION} is not set; this is not an agent run"))?;
    let token = env_value(ENV_TOKEN).ok_or_else(|| format!("{ENV_TOKEN} is not set"))?;
    let payload = match payload {
        Some(payload) => Some(payload),
        None if io::stdin().is_terminal() => None,
        None => read_stdin(deadline)?,
    };
    if payload
        .as_ref()
        .is_some_and(|payload| payload.len() > MAX_HOOK_PAYLOAD_BYTES)
    {
        return Err(format!(
            "the payload is larger than {MAX_HOOK_PAYLOAD_BYTES} bytes"
        ));
    }
    let profile = Profile::resolve(home, profile).map_err(|error| error.to_string())?;
    let params = AgentHookParams {
        session,
        token,
        event,
        payload,
    };
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    let outcome = runtime.block_on(async move {
        let call = async {
            let client = connect_running(&profile)
                .await
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "the service is not running".to_owned())?;
            client
                .agent_hook(&params)
                .await
                .map_err(|error| error.to_string())
        };
        tokio::time::timeout(remaining, call)
            .await
            .map_err(|_| format!("the service did not answer within {} s", BUDGET.as_secs()))?
    });
    // A connect still running on the blocking pool (it can wait without a bound on Windows) must not hold the
    // command past its budget: the runtime is dropped without waiting for it.
    runtime.shutdown_background();
    outcome
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// Reads stdin to its end on a helper thread, so a pipe that never closes cannot outlast the budget. Empty input is
/// no payload.
fn read_stdin(deadline: Instant) -> Result<Option<String>, String> {
    let (sender, receiver) = mpsc::channel();
    let limit = u64::try_from(MAX_HOOK_PAYLOAD_BYTES + 1).unwrap_or(u64::MAX);
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let read = io::stdin().lock().take(limit).read_to_end(&mut bytes);
        let _ = sender.send(read.map(|_| bytes));
    });
    let remaining = deadline.saturating_duration_since(Instant::now());
    let bytes = receiver
        .recv_timeout(remaining)
        .map_err(|_| "stdin did not end within the budget".to_owned())?
        .map_err(|error| format!("cannot read stdin: {error}"))?;
    if bytes.is_empty() {
        return Ok(None);
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "the payload on stdin is not UTF-8".to_owned())
}
