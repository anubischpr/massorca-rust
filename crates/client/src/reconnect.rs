//! A stream that outlives its connection (decision 0040): when the connection drops, it reconnects with bounded,
//! jittered exponential backoff and opens the stream again. Nothing new goes over the wire; each attempt is an
//! ordinary connect and subscribe, so it works with any service.

use std::future::Future;
use std::time::{Duration, Instant};

use massorca_protocol::{ErrorBody, codes};

use crate::client::Client;
use crate::error::ClientError;
use crate::subscription::{StreamItem, Subscription};

/// The first pause of the network client (decision 0040).
pub const RECONNECT_START: Duration = Duration::from_millis(250);
/// The longest pause of the network client (decision 0040).
pub const RECONNECT_MAX: Duration = Duration::from_secs(30);
/// A connection that stayed up this long starts the backoff over (decision 0040).
pub const RECONNECT_RESET_AFTER: Duration = Duration::from_secs(60);

/// Pauses between attempts: `start`, doubling up to `max`. A success starts it over.
#[derive(Debug, Clone)]
pub struct Backoff {
    start: Duration,
    max: Duration,
    failures: u32,
}

impl Backoff {
    pub const fn new(start: Duration, max: Duration) -> Self {
        Self {
            start,
            max,
            failures: 0,
        }
    }

    pub fn reset(&mut self) {
        self.failures = 0;
    }

    /// Counts one more failure and returns the pause before the next attempt.
    pub fn next_delay(&mut self) -> Duration {
        self.failures = self.failures.saturating_add(1);
        let doublings = (self.failures - 1).min(16);
        self.start.saturating_mul(1u32 << doublings).min(self.max)
    }

    pub fn failures(&self) -> u32 {
        self.failures
    }
}

impl Default for Backoff {
    /// The network client's backoff: 250 ms doubling to 30 s.
    fn default() -> Self {
        Self::new(RECONNECT_START, RECONNECT_MAX)
    }
}

/// `delay` with "equal jitter": half of it fixed, the other half random, so clients that lost the same server do not
/// all come back at once.
pub fn jittered(delay: Duration) -> Duration {
    let half = delay / 2;
    let mut random = [0u8; 4];
    if getrandom::fill(&mut random).is_err() {
        return delay;
    }
    let fraction = f64::from(u32::from_le_bytes(random)) / f64::from(u32::MAX);
    half + half.mul_f64(fraction)
}

/// Whether trying again later may succeed: the service was unreachable, stopping, or the connection dropped. A
/// refusal (a revoked device, a changed server key, a missing permission) is final.
pub fn is_transient(error: &ClientError) -> bool {
    match error {
        ClientError::Disconnected
        | ClientError::Unavailable(_)
        | ClientError::HostUnreachable(_)
        | ClientError::Io(_) => true,
        ClientError::Remote(body) => is_transient_body(body),
        ClientError::Handshake { error, .. } => is_transient_body(error),
        _ => false,
    }
}

fn is_transient_body(body: &ErrorBody) -> bool {
    body.code == codes::SERVICE_SHUTTING_DOWN
}

/// How to reach the service and open the stream, once per attempt.
pub trait Reopen {
    fn connect(&mut self) -> impl Future<Output = Result<Client, ClientError>> + Send;
    fn open(
        &mut self,
        client: &Client,
    ) -> impl Future<Output = Result<Subscription, ClientError>> + Send;
}

/// Whether what the stream reports can be trusted right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    /// Connected again after a loss, with the stream opened afresh.
    Live,
    /// Out of contact: the last state seen may have changed, which says nothing about whether anything ended.
    Unverifiable {
        /// The attempt that runs next, from 1.
        attempt: u32,
        retry_in: Duration,
        reason: String,
    },
}

/// One item of a resilient stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resilient {
    Item(StreamItem),
    Link(Link),
}

/// A stream that reconnects when its connection drops. The first connection is made by the first `next`.
pub struct ResilientSubscription<R> {
    reopen: R,
    backoff: Backoff,
    current: Option<Current>,
    retry_at: Option<Instant>,
    /// A loss was reported and `Live` has not followed yet.
    lost: bool,
}

impl<R> std::fmt::Debug for ResilientSubscription<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResilientSubscription")
            .field("backoff", &self.backoff)
            .field("connected", &self.current.is_some())
            .field("lost", &self.lost)
            .finish_non_exhaustive()
    }
}

struct Current {
    // Kept with the stream so the connection stays open while it is read.
    _client: Client,
    subscription: Subscription,
    since: Instant,
}

impl<R: Reopen> ResilientSubscription<R> {
    pub fn new(reopen: R, backoff: Backoff) -> Self {
        Self {
            reopen,
            backoff,
            current: None,
            retry_at: None,
            lost: false,
        }
    }

    /// The next item or link change. `Some(Err)` is final: a refusal, or a stream the service ended with an error that
    /// a retry would not fix. `None` once the service ended the stream normally.
    pub async fn next(&mut self) -> Option<Result<Resilient, ClientError>> {
        loop {
            let Some(current) = self.current.as_mut() else {
                if let Some(at) = self.retry_at.take() {
                    tokio::time::sleep_until(at.into()).await;
                }
                match self.attempt().await {
                    Ok(current) => {
                        self.current = Some(current);
                        if std::mem::take(&mut self.lost) {
                            return Some(Ok(Resilient::Link(Link::Live)));
                        }
                        continue;
                    }
                    Err(error) if is_transient(&error) => return Some(Ok(self.lose(&error))),
                    Err(error) => return Some(Err(error)),
                }
            };
            let lost = match current.subscription.next_item().await {
                Some(Ok(item)) => return Some(Ok(Resilient::Item(item))),
                Some(Err(body)) if is_transient_body(&body) => ClientError::Remote(body),
                Some(Err(body)) => return Some(Err(ClientError::Remote(body))),
                None if current.subscription.connection_lost() => ClientError::Disconnected,
                None => return None,
            };
            if current.since.elapsed() >= RECONNECT_RESET_AFTER {
                self.backoff.reset();
            }
            self.current = None;
            return Some(Ok(self.lose(&lost)));
        }
    }

    async fn attempt(&mut self) -> Result<Current, ClientError> {
        let client = self.reopen.connect().await?;
        let subscription = self.reopen.open(&client).await?;
        Ok(Current {
            _client: client,
            subscription,
            since: Instant::now(),
        })
    }

    /// Schedules the next attempt and says so.
    fn lose(&mut self, error: &ClientError) -> Resilient {
        let retry_in = jittered(self.backoff.next_delay());
        self.retry_at = Some(Instant::now() + retry_in);
        self.lost = true;
        Resilient::Link(Link::Unverifiable {
            attempt: self.backoff.failures(),
            retry_in,
            reason: error.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_network_backoff_runs_from_250_ms_to_30_s() {
        let mut backoff = Backoff::default();
        let delays: Vec<u128> = (0..9).map(|_| backoff.next_delay().as_millis()).collect();
        assert_eq!(
            delays,
            vec![250, 500, 1000, 2000, 4000, 8000, 16000, 30000, 30000]
        );
        backoff.reset();
        assert_eq!(backoff.next_delay(), RECONNECT_START);
    }

    #[test]
    fn the_backoff_never_overflows() {
        let mut backoff = Backoff::default();
        for _ in 0..100 {
            assert!(backoff.next_delay() <= RECONNECT_MAX);
        }
        assert_eq!(backoff.failures(), 100);
    }

    #[test]
    fn jitter_keeps_between_half_and_all_of_the_delay() {
        let delay = Duration::from_secs(8);
        let delays: Vec<Duration> = (0..200).map(|_| jittered(delay)).collect();
        assert!(delays.iter().all(|d| *d >= delay / 2 && *d <= delay));
        let first = delays[0];
        assert!(delays.iter().any(|d| *d != first), "the delays vary");
    }

    #[test]
    fn only_losses_are_retried_and_refusals_are_final() {
        let shutting_down = ErrorBody::new(codes::SERVICE_SHUTTING_DOWN, "stopping");
        let revoked = ErrorBody::new(codes::AUTH_DEVICE_REVOKED, "revoked");
        assert!(is_transient(&ClientError::Disconnected));
        assert!(is_transient(&ClientError::HostUnreachable("x".to_owned())));
        assert!(is_transient(&ClientError::Remote(shutting_down)));
        assert!(!is_transient(&ClientError::Remote(revoked.clone())));
        assert!(!is_transient(&ClientError::Handshake {
            error: Box::new(revoked),
            supported: None,
        }));
        assert!(!is_transient(&ClientError::ServerIdentityMismatch));
    }
}
