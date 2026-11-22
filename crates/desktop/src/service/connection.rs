//! The connection states the shell shows, and the backoff between attempts (decision 0028).

use std::time::{Duration, Instant};

use massorca_client::ClientError;

/// The first pause after a failed attempt. It doubles up to [`BACKOFF_MAX`].
pub const BACKOFF_START: Duration = Duration::from_millis(250);
pub const BACKOFF_MAX: Duration = Duration::from_secs(5);

/// Where the link to the service stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    /// The first attempt of this run is under way.
    Connecting,
    Connected {
        server: String,
        version: String,
    },
    /// The connection was lost; attempt `attempt` runs at `retry_at`.
    Reconnecting {
        attempt: u32,
        retry_at: Instant,
    },
    /// The first attempt failed. The bridge keeps trying, and "Retry now" tries at once.
    Unreachable {
        reason: String,
    },
}

impl ConnectionState {
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }
}

/// Pauses between attempts: 250 ms, doubling up to 5 s. A success or "Retry now" starts it over.
#[derive(Debug, Default)]
pub struct Backoff {
    failures: u32,
}

impl Backoff {
    pub fn reset(&mut self) {
        self.failures = 0;
    }

    /// Counts one more failure and returns the pause before the next attempt.
    pub fn next_delay(&mut self) -> Duration {
        self.failures = self.failures.saturating_add(1);
        let doublings = (self.failures - 1).min(16);
        BACKOFF_START
            .saturating_mul(1u32 << doublings)
            .min(BACKOFF_MAX)
    }

    pub fn failures(&self) -> u32 {
        self.failures
    }
}

/// The state after a connected link dropped, and the pause before the first retry.
pub fn after_loss(backoff: &mut Backoff, now: Instant) -> (ConnectionState, Duration) {
    backoff.reset();
    let delay = backoff.next_delay();
    let state = ConnectionState::Reconnecting {
        attempt: backoff.failures(),
        retry_at: now + delay,
    };
    (state, delay)
}

/// The state after an attempt failed. Before the link was ever up the shell says the service is
/// unreachable; after a loss it keeps counting reconnect attempts.
pub fn after_failed_attempt(
    link_was_up: bool,
    error: &ClientError,
    backoff: &mut Backoff,
    now: Instant,
) -> (ConnectionState, Duration) {
    let delay = backoff.next_delay();
    let state = if link_was_up {
        ConnectionState::Reconnecting {
            attempt: backoff.failures(),
            retry_at: now + delay,
        }
    } else {
        ConnectionState::Unreachable {
            reason: reason_text(error),
        }
    };
    (state, delay)
}

/// A sentence for why the service could not be reached, for the banner and its details.
pub fn reason_text(error: &ClientError) -> String {
    match error {
        ClientError::Unavailable(_) => "The service is not running.".to_owned(),
        ClientError::StartFailed(why) => format!("The service could not be started: {why}"),
        ClientError::UntrustedEndpoint(why) => {
            format!("The service endpoint is not trusted: {why}")
        }
        ClientError::Handshake { error, .. } => {
            format!("The service refused the connection: {}", error.message)
        }
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    fn unavailable() -> ClientError {
        ClientError::Unavailable(io::Error::from(io::ErrorKind::NotFound))
    }

    #[test]
    fn backoff_starts_at_250_ms_doubles_and_stops_at_5_s() {
        let mut backoff = Backoff::default();
        let delays: Vec<u128> = (0..7).map(|_| backoff.next_delay().as_millis()).collect();
        assert_eq!(delays, vec![250, 500, 1000, 2000, 4000, 5000, 5000]);
    }

    #[test]
    fn retry_now_starts_the_backoff_over() {
        let mut backoff = Backoff::default();
        backoff.next_delay();
        backoff.next_delay();
        backoff.reset();
        assert_eq!(backoff.next_delay(), BACKOFF_START);
    }

    #[test]
    fn backoff_never_overflows() {
        let mut backoff = Backoff::default();
        for _ in 0..100 {
            assert!(backoff.next_delay() <= BACKOFF_MAX);
        }
    }

    #[test]
    fn a_failed_first_attempt_is_unreachable_with_a_reason() {
        let mut backoff = Backoff::default();
        let (state, delay) =
            after_failed_attempt(false, &unavailable(), &mut backoff, Instant::now());
        assert_eq!(
            state,
            ConnectionState::Unreachable {
                reason: "The service is not running.".into()
            }
        );
        assert_eq!(delay, BACKOFF_START);
    }

    #[test]
    fn a_lost_link_counts_reconnect_attempts_with_a_retry_time() {
        let mut backoff = Backoff::default();
        let now = Instant::now();
        let (state, delay) = after_loss(&mut backoff, now);
        assert_eq!(
            state,
            ConnectionState::Reconnecting {
                attempt: 1,
                retry_at: now + BACKOFF_START
            }
        );
        assert_eq!(delay, BACKOFF_START);
        let (state, delay) = after_failed_attempt(true, &unavailable(), &mut backoff, now);
        assert_eq!(
            state,
            ConnectionState::Reconnecting {
                attempt: 2,
                retry_at: now + delay
            }
        );
        assert_eq!(delay.as_millis(), 500);
    }

    #[test]
    fn only_a_connected_state_is_connected() {
        let up = ConnectionState::Connected {
            server: "massorca".into(),
            version: "0.1.0".into(),
        };
        assert!(up.is_connected());
        assert!(!ConnectionState::Connecting.is_connected());
    }
}
