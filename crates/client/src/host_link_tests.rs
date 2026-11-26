//! The host link against scripted connections: each attempt takes the next scripted outcome, and a connection ends
//! when the test says so.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use massorca_protocol::{MAX_FRAME_LEN_LOCAL, PeerInfo};
use tokio::sync::{Notify, oneshot, watch};

use super::*;
use crate::handshake::Welcome;
use crate::identity_mismatch::{IdentityMismatch, TrustedFrom};
use crate::transport::FrameIo;

const WAIT: Duration = Duration::from_secs(10);

/// A connection whose far end hangs up when the returned sender fires or is dropped.
fn connection() -> (Client, oneshot::Sender<()>) {
    let (hang_up, hung_up) = oneshot::channel::<()>();
    let io = FrameIo::spawn_with(MAX_FRAME_LEN_LOCAL, |queues| {
        let inbound = queues.inbound;
        let mut outbound = queues.outbound;
        let reader = tokio::spawn(async move {
            let _ = hung_up.await;
            drop(inbound);
        });
        let writer = tokio::spawn(async move { while outbound.recv().await.is_some() {} });
        (reader, writer)
    });
    let welcome = Welcome {
        protocol: 1,
        server: PeerInfo {
            name: "massorca-test".to_owned(),
            version: "0.0.0-test".to_owned(),
        },
        capabilities: Vec::new(),
        connection_id: "c1".to_owned(),
        max_frame_len: MAX_FRAME_LEN_LOCAL,
    };
    (Client::open(io, welcome), hang_up)
}

/// Attempts take scripted outcomes in order; with none left, the host is unreachable.
#[derive(Clone, Default)]
struct Script {
    outcomes: Arc<Mutex<VecDeque<Result<Client, ClientError>>>>,
    attempts: Arc<AtomicU32>,
}

impl Script {
    fn push(&self, outcome: Result<Client, ClientError>) {
        self.outcomes.lock().expect("script").push_back(outcome);
    }

    fn attempts(&self) -> u32 {
        self.attempts.load(Ordering::SeqCst)
    }
}

impl Connect for Script {
    async fn connect(&mut self) -> Result<Client, ClientError> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        let next = self.outcomes.lock().expect("script").pop_front();
        next.unwrap_or_else(|| Err(ClientError::HostUnreachable("refused".to_owned())))
    }
}

fn fast() -> Backoff {
    Backoff::new(Duration::from_millis(5), Duration::from_millis(20))
}

fn mismatch() -> ClientError {
    ClientError::ServerIdentityMismatch(Box::new(IdentityMismatch {
        host: "vps".to_owned(),
        trusted_from: TrustedFrom::Pairing,
        expected: "sha256:old".to_owned(),
        actual: Some("sha256:new".to_owned()),
    }))
}

async fn until(
    states: &mut watch::Receiver<LiveState>,
    what: &str,
    done: impl Fn(&LiveState) -> bool,
) {
    let reached = tokio::time::timeout(WAIT, states.wait_for(|state| done(state)))
        .await
        .is_ok_and(|waited| waited.is_ok());
    assert!(
        reached,
        "timed out waiting for {what}: {:?}",
        *states.borrow()
    );
}

#[tokio::test]
async fn a_dropped_connection_is_unreachable_then_connected_again() {
    let script = Script::default();
    let (first, hang_up) = connection();
    script.push(Ok(first));
    script.push(Err(ClientError::HostUnreachable("refused".to_owned())));
    let (second, _keep) = connection();
    script.push(Ok(second));
    let link = LiveHost::start(script.clone(), fast());
    let mut states = link.subscribe();
    until(&mut states, "connected", LiveState::is_connected).await;

    hang_up.send(()).expect("the far end hangs up");
    until(&mut states, "unreachable", |state| {
        matches!(state, LiveState::Unreachable { attempt: 1, error, .. } if matches!(**error, ClientError::Disconnected))
    })
    .await;
    until(&mut states, "connected again", LiveState::is_connected).await;
    assert_eq!(
        script.attempts(),
        3,
        "one failed retry, then the host answered"
    );
}

#[tokio::test]
async fn a_changed_server_key_is_final_until_reconnect_is_asked() {
    let script = Script::default();
    script.push(Err(mismatch()));
    let link = LiveHost::start(script.clone(), fast());
    let mut states = link.subscribe();
    until(&mut states, "ended", |state| {
        matches!(state, LiveState::Ended { .. })
    })
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        script.attempts(),
        1,
        "a changed key is never retried by itself"
    );

    let (client, _keep) = connection();
    script.push(Ok(client));
    link.reconnect();
    until(&mut states, "connected", LiveState::is_connected).await;
    assert_eq!(script.attempts(), 2);
}

#[tokio::test]
async fn reconnect_tries_at_once_instead_of_waiting_for_the_backoff() {
    let script = Script::default();
    let slow = Backoff::new(Duration::from_secs(3600), Duration::from_secs(3600));
    let link = LiveHost::start(script.clone(), slow);
    let mut states = link.subscribe();
    until(&mut states, "unreachable", |state| {
        matches!(state, LiveState::Unreachable { .. })
    })
    .await;

    let (client, _keep) = connection();
    script.push(Ok(client));
    link.reconnect();
    until(&mut states, "connected", LiveState::is_connected).await;
}

#[tokio::test]
async fn reconnect_on_a_live_connection_opens_a_new_one() {
    let script = Script::default();
    let (first, _keep_first) = connection();
    script.push(Ok(first));
    let link = LiveHost::start(script.clone(), fast());
    let mut states = link.subscribe();
    until(&mut states, "connected", LiveState::is_connected).await;

    let (second, _keep_second) = connection();
    script.push(Ok(second));
    link.reconnect();
    tokio::time::timeout(WAIT, async {
        while script.attempts() < 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("a second attempt runs");
    until(&mut states, "connected", LiveState::is_connected).await;
}

#[tokio::test]
async fn dropping_the_link_stops_its_attempts() {
    let script = Script::default();
    let link = LiveHost::start(script.clone(), fast());
    let mut states = link.subscribe();
    until(&mut states, "unreachable", |state| {
        matches!(state, LiveState::Unreachable { .. })
    })
    .await;
    drop(link);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let stopped_at = script.attempts();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        script.attempts(),
        stopped_at,
        "no attempt runs after the link is dropped"
    );
}

#[tokio::test]
async fn dropping_the_link_closes_the_connection_its_borrowers_hold() {
    let script = Script::default();
    let (first, _keep) = connection();
    let probe = first.clone();
    script.push(Ok(first));
    let link = LiveHost::start(script.clone(), fast());
    let mut states = link.subscribe();
    until(&mut states, "connected", LiveState::is_connected).await;
    let mut borrower = link.watch();
    drop(link);

    tokio::time::timeout(WAIT, probe.closed())
        .await
        .expect("the connection ends with its link");
    let stopped = borrower.client().await.expect_err("the link is gone");
    assert!(matches!(*stopped, ClientError::Disconnected), "{stopped:?}");
}

#[tokio::test]
async fn an_open_connection_is_lent_whatever_state_the_link_reads() {
    let (client, _keep) = connection();
    let (_state_tx, state) = watch::channel(LiveState::Unreachable {
        error: Arc::new(ClientError::Disconnected),
        attempt: 1,
        retry_in: Duration::from_secs(1),
    });
    let (_connection_tx, connection) = watch::channel(Some(client));
    let mut watch = LiveWatch { state, connection };
    watch
        .client()
        .await
        .expect("the open connection is lent, not the state read before a reconnect");
}

#[tokio::test]
async fn a_closed_connection_is_not_lent() {
    let (closed, _keep) = connection();
    closed.close();
    tokio::time::timeout(WAIT, closed.closed())
        .await
        .expect("the connection ends");
    let (_state_tx, state) = watch::channel(LiveState::Connected {
        server: "massorca-test".to_owned(),
        version: "0.0.0-test".to_owned(),
    });
    let (_connection_tx, connection) = watch::channel(Some(closed));
    let mut watch = LiveWatch { state, connection };
    let stopped = watch
        .client()
        .await
        .expect_err("a closed connection is not lent");
    assert!(matches!(*stopped, ClientError::Disconnected), "{stopped:?}");
}

#[tokio::test]
async fn ending_the_lending_closes_and_clears_the_connection_it_lent() {
    let (client, _keep) = connection();
    let probe = client.clone();
    let (sender, held) = watch::channel(None);
    let lending = Lending(sender);
    lending.lend(Some(client));
    drop(lending);

    assert!(held.borrow().is_none(), "the lent connection is cleared");
    tokio::time::timeout(WAIT, probe.closed())
        .await
        .expect("the lent connection is closed");
}

#[tokio::test]
async fn retry_now_shows_connecting_at_once_not_unreachable() {
    let script = Script::default();
    let slow = Backoff::new(Duration::from_secs(3600), Duration::from_secs(3600));
    let link = LiveHost::start(script.clone(), slow);
    let mut states = link.subscribe();
    until(&mut states, "unreachable", |state| {
        matches!(state, LiveState::Unreachable { .. })
    })
    .await;

    link.reconnect();
    assert!(
        matches!(link.state(), LiveState::Connecting),
        "{:?}",
        link.state()
    );
}

/// Fails its first attempt once `gate` opens, then connects with `next`.
#[derive(Clone)]
struct Gated {
    gate: Arc<Notify>,
    attempts: Arc<AtomicU32>,
    next: Arc<Mutex<Option<Client>>>,
}

impl Connect for Gated {
    async fn connect(&mut self) -> Result<Client, ClientError> {
        if self.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
            self.gate.notified().await;
            return Err(ClientError::HostUnreachable("refused".to_owned()));
        }
        self.next
            .lock()
            .expect("the slot")
            .take()
            .ok_or_else(|| ClientError::HostUnreachable("refused".to_owned()))
    }
}

#[tokio::test]
async fn a_retry_asked_during_a_failed_attempt_runs_at_once() {
    let (client, _keep) = connection();
    let gate = Arc::new(Notify::new());
    let connect = Gated {
        gate: Arc::clone(&gate),
        attempts: Arc::default(),
        next: Arc::new(Mutex::new(Some(client))),
    };
    let attempts = Arc::clone(&connect.attempts);
    let hour = Duration::from_secs(3600);
    let link = LiveHost::start(connect, Backoff::new(hour, hour));
    let mut states = link.subscribe();
    tokio::time::timeout(WAIT, async {
        while attempts.load(Ordering::SeqCst) < 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the first attempt runs");

    link.reconnect();
    gate.notify_one();
    until(&mut states, "connected", LiveState::is_connected).await;
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        2,
        "the retry did not wait for the backoff"
    );
}
