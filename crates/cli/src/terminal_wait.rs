//! `terminal wait`: blocks until a terminal's process exits. The service answers each wait within a minute, so the
//! command sends the wait again until the caller's own timeout ends it.

use std::time::{Duration, Instant};

use massorca_client::{Client, ClientError};
use massorca_protocol::methods::terminals::{
    MAX_WAIT_MS, TerminalState, TerminalWait, TerminalWaitCondition, TerminalWaitParams,
};
use massorca_protocol::{capabilities, codes};
use serde_json::json;
use tokio::time::timeout;

use crate::error::{CliError, exit};
use crate::terminal::{TerminalWaitArgs, WaitCondition, exit_text, state_text};

/// How long a reply may take beyond the wait it was sent with before the service counts as not answering.
const REPLY_GRACE: Duration = Duration::from_secs(10);

/// Waits until the terminal's process exits or `--timeout` seconds pass. Each call to the service waits at most the
/// service's cap, so the wait is sent again until the caller's own timeout ends it.
pub async fn run(client: &Client, args: &TerminalWaitArgs) -> Result<TerminalWait, CliError> {
    require_capability(client.has_capability(capabilities::TERMINAL_WAIT))?;
    let condition = match args.condition {
        WaitCondition::Exit => TerminalWaitCondition::Exit,
    };
    let deadline = Instant::now().checked_add(Duration::from_secs(args.timeout));
    loop {
        let slice = slice_for(deadline, Instant::now());
        let params = TerminalWaitParams {
            terminal: args.terminal.clone(),
            condition,
            timeout_ms: Some(u64::try_from(slice.as_millis()).unwrap_or(MAX_WAIT_MS)),
        };
        let answer = match timeout(slice + REPLY_GRACE, client.terminal_wait(&params)).await {
            Ok(Ok(answer)) => answer,
            Ok(Err(error)) => return Err(wait_error(error, &args.terminal)),
            Err(_elapsed) => return Err(wait_error(ClientError::Disconnected, &args.terminal)),
        };
        if answer.satisfied {
            return Ok(answer);
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(timed_out(&args.terminal, args.timeout, answer.status));
        }
    }
}

/// Fails with the capability error when the service does not offer `terminal-wait/1`.
pub fn require_capability(offered: bool) -> Result<(), CliError> {
    if offered {
        return Ok(());
    }
    Err(CliError::new(
        codes::CAPABILITY_NOT_NEGOTIATED,
        format!(
            "this service does not offer {}; update MassOrca on the host that runs the terminal",
            capabilities::TERMINAL_WAIT
        ),
        exit::ERROR,
    ))
}

/// The error for a wait whose answer did not arrive. A refusal the service sent keeps its own code.
pub fn wait_error(error: ClientError, terminal: &str) -> CliError {
    match error {
        ClientError::Remote(body) => CliError::from(ClientError::Remote(body)),
        _ => CliError::new(
            "terminal.unverifiable",
            format!(
                "lost contact with the service while waiting for terminal {terminal}; its exit is unknown and it may still be running"
            ),
            exit::UNAVAILABLE,
        ),
    }
}

/// The longest one wait call may run: the time left until `deadline`, capped at the service's cap.
pub fn slice_for(deadline: Option<Instant>, now: Instant) -> Duration {
    let cap = Duration::from_millis(MAX_WAIT_MS);
    match deadline {
        Some(deadline) => deadline.saturating_duration_since(now).min(cap),
        None => cap,
    }
}

/// The failure of a wait whose own timeout passed; the state the terminal was in is named.
fn timed_out(terminal: &str, seconds: u64, status: TerminalState) -> CliError {
    let state = state_text(status);
    CliError {
        code: "terminal.wait_timeout".to_owned(),
        message: format!("terminal {terminal} did not exit within {seconds} s; it is {state}"),
        data: Some(Box::new(json!({
            "terminal": terminal,
            "status": state,
            "timeout_seconds": seconds,
        }))),
        exit_code: exit::ERROR,
    }
}

/// The text output: the terminal, its state, and how it ended once it has.
pub fn lines(answer: &TerminalWait) -> Vec<String> {
    let mut lines = vec![
        format!("terminal: {}", answer.terminal),
        format!("state: {}", state_text(answer.status)),
    ];
    if let Some(exit) = &answer.exit {
        lines.push(format!("exit: {}", exit_text(exit)));
    }
    lines
}

#[cfg(test)]
mod tests {
    use massorca_protocol::ErrorBody;

    use super::*;

    #[test]
    fn a_service_without_the_capability_is_refused_by_name() {
        let error = require_capability(false).expect_err("the wait needs the capability");
        assert_eq!(error.code, codes::CAPABILITY_NOT_NEGOTIATED);
        assert!(
            error.message.contains("terminal-wait/1"),
            "{}",
            error.message
        );
        require_capability(true).expect("a service that offers it is used");
    }

    #[test]
    fn a_lost_connection_is_unverifiable_and_never_exited() {
        let error = wait_error(ClientError::Disconnected, "t-1");
        assert_eq!(error.code, "terminal.unverifiable");
        assert_eq!(error.exit_code, exit::UNAVAILABLE);
        assert!(
            error.message.contains("may still be running"),
            "{}",
            error.message
        );
    }

    #[test]
    fn a_refusal_from_the_service_keeps_its_own_code() {
        let body = ErrorBody::new(codes::TERMINAL_GONE, "closed during the wait");
        let error = wait_error(ClientError::Remote(body), "t-1");
        assert_eq!(error.code, codes::TERMINAL_GONE);
    }

    #[test]
    fn each_call_waits_for_the_time_left_capped_at_the_service_cap() {
        let now = Instant::now();
        assert_eq!(
            slice_for(Some(now + Duration::from_secs(5)), now),
            Duration::from_secs(5)
        );
        assert_eq!(
            slice_for(Some(now + Duration::from_secs(600)), now),
            Duration::from_millis(MAX_WAIT_MS)
        );
        assert_eq!(slice_for(None, now), Duration::from_millis(MAX_WAIT_MS));
        assert_eq!(
            slice_for(Some(now), now + Duration::from_secs(1)),
            Duration::ZERO
        );
    }
}
