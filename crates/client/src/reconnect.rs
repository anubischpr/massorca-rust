//! A stream that outlives its connection (decision 0040): when the connection drops, it reconnects with bounded,
//! jittered exponential backoff and opens the stream again. Nothing new goes over the wire; each attempt is an
//! ordinary connect and subscribe, so it works with any service.

use std::future::Future;
use std::time::{Duration, Instant};

use massorca_protocol::methods::terminals::TerminalAttachParams;
use massorca_protocol::{ErrorBody, codes};

use crate::client::Client;
use crate::error::ClientError;
use crate::subscription::{StreamItem, Subscription};
use crate::terminal_attach::{AttachUpdate, TerminalAttachment};

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

/// How to reach the service again, once per attempt.
pub trait Connect {
    fn connect(&mut self) -> impl Future<Output = Result<Client, ClientError>> + Send;
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

/// The pacing shared by every resilient stream: the backoff, when the next attempt may run, and whether a loss was
/// reported that `Live` has not answered yet.
#[derive(Debug)]
struct Retry {
    backoff: Backoff,
    retry_at: Option<Instant>,
    lost: bool,
}

impl Retry {
    fn new(backoff: Backoff) -> Self {
        Self {
            backoff,
            retry_at: None,
            lost: false,
        }
    }

    /// Cancel-safe: a wait cut short by another branch of a `select!` keeps its schedule.
    async fn wait(&mut self) {
        if let Some(at) = self.retry_at {
            tokio::time::sleep_until(at.into()).await;
            self.retry_at = None;
        }
    }

    /// `Live` when a loss was reported since the last connection.
    fn back(&mut self) -> Option<Link> {
        std::mem::take(&mut self.lost).then_some(Link::Live)
    }

    /// Schedules the next attempt and says so. A connection that had been up long enough starts the backoff over.
    fn lose(&mut self, error: &ClientError, connected_since: Option<Instant>) -> Link {
        if connected_since.is_some_and(|since| since.elapsed() >= RECONNECT_RESET_AFTER) {
            self.backoff.reset();
        }
        let retry_in = jittered(self.backoff.next_delay());
        self.retry_at = Some(Instant::now() + retry_in);
        self.lost = true;
        Link::Unverifiable {
            attempt: self.backoff.failures(),
            retry_in,
            reason: error.to_string(),
        }
    }
}

/// A stream that reconnects when its connection drops. The first connection is made by the first `next`.
pub struct ResilientSubscription<R> {
    reopen: R,
    retry: Retry,
    current: Option<Current>,
}

impl<R> std::fmt::Debug for ResilientSubscription<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResilientSubscription")
            .field("retry", &self.retry)
            .field("connected", &self.current.is_some())
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
            retry: Retry::new(backoff),
            current: None,
        }
    }

    /// The next item or link change. `Some(Err)` is final: a refusal, or a stream the service ended with an error that
    /// a retry would not fix. `None` once the service ended the stream normally.
    pub async fn next(&mut self) -> Option<Result<Resilient, ClientError>> {
        loop {
            let Some(current) = self.current.as_mut() else {
                self.retry.wait().await;
                match self.attempt().await {
                    Ok(current) => {
                        self.current = Some(current);
                        if let Some(link) = self.retry.back() {
                            return Some(Ok(Resilient::Link(link)));
                        }
                        continue;
                    }
                    Err(error) if is_transient(&error) => {
                        return Some(Ok(Resilient::Link(self.retry.lose(&error, None))));
                    }
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
            let since = current.since;
            self.current = None;
            return Some(Ok(Resilient::Link(self.retry.lose(&lost, Some(since)))));
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
}

/// One item of a resilient terminal attach.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttachStep {
    Update(AttachUpdate),
    Link(Link),
}

/// A terminal attach that reconnects when its connection drops and resumes after the output already read, so nothing
/// is shown twice. When the service no longer holds that output it redraws the screen with a snapshot instead
/// (decision 0035). The first connection is made by the first `next`.
pub struct ResilientAttach<C> {
    connect: C,
    params: TerminalAttachParams,
    retry: Retry,
    current: Option<(Client, TerminalAttachment, Instant)>,
    /// The output offset read so far, once output has been read.
    read: Option<u64>,
}

impl<C> std::fmt::Debug for ResilientAttach<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResilientAttach")
            .field("terminal", &self.params.terminal)
            .field("read", &self.read)
            .field("connected", &self.current.is_some())
            .finish_non_exhaustive()
    }
}

impl<C: Connect> ResilientAttach<C> {
    pub fn new(connect: C, params: TerminalAttachParams, backoff: Backoff) -> Self {
        let read = params.since;
        Self {
            connect,
            params,
            retry: Retry::new(backoff),
            current: None,
            read,
        }
    }

    /// The client of the live connection, for input and resizes. `None` while out of contact.
    pub fn client(&self) -> Option<&Client> {
        self.current.as_ref().map(|(client, _, _)| client)
    }

    /// The size a reattach applies, kept current as the local terminal is resized.
    pub fn set_size(&mut self, cols: u16, rows: u16) {
        self.params.cols = Some(cols);
        self.params.rows = Some(rows);
    }

    /// The next update or link change. `Some(Err)` is final; `None` once the service ended the attach normally.
    pub async fn next(&mut self) -> Option<Result<AttachStep, ClientError>> {
        loop {
            let Some((_, attachment, since)) = self.current.as_mut() else {
                self.retry.wait().await;
                match self.attempt().await {
                    Ok(current) => {
                        self.current = Some(current);
                        if let Some(link) = self.retry.back() {
                            return Some(Ok(AttachStep::Link(link)));
                        }
                        continue;
                    }
                    Err(error) if is_transient(&error) => {
                        return Some(Ok(AttachStep::Link(self.retry.lose(&error, None))));
                    }
                    Err(error) => return Some(Err(error)),
                }
            };
            let lost = match attachment.next().await {
                Some(Ok(update)) => {
                    if matches!(
                        update,
                        AttachUpdate::Output { .. } | AttachUpdate::Snapshot { .. }
                    ) {
                        self.read = Some(attachment.next_offset());
                    }
                    return Some(Ok(AttachStep::Update(update)));
                }
                Some(Err(ClientError::Remote(body))) if is_transient_body(&body) => {
                    ClientError::Remote(body)
                }
                Some(Err(error)) => return Some(Err(error)),
                None if attachment.connection_lost() => ClientError::Disconnected,
                None => return None,
            };
            let since = *since;
            self.current = None;
            return Some(Ok(AttachStep::Link(self.retry.lose(&lost, Some(since)))));
        }
    }

    async fn attempt(&mut self) -> Result<(Client, TerminalAttachment, Instant), ClientError> {
        let client = self.connect.connect().await?;
        let params = TerminalAttachParams {
            since: self.read,
            ..self.params.clone()
        };
        let attachment = client.terminal_attach(&params).await?;
        Ok((client, attachment, Instant::now()))
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
        let changed = crate::IdentityMismatch {
            host: "vps".to_owned(),
            trusted_from: crate::TrustedFrom::Pairing,
            expected: "sha256:old".to_owned(),
            actual: None,
        };
        assert!(!is_transient(&ClientError::ServerIdentityMismatch(
            Box::new(changed)
        )));
    }
}
