//! The live links against scripted hosts that refuse or never answer, so each attempt is counted.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use massorca_client::{Client, ClientError, Connect};

use super::*;

/// Never reaches its host; counts the attempts made for it.
#[derive(Clone, Default)]
struct Unreachable(Arc<AtomicU32>);

impl Connect for Unreachable {
    async fn connect(&mut self) -> Result<Client, ClientError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(ClientError::HostUnreachable("refused".to_owned()))
    }
}

/// Links whose retries wait an hour, so only `reconnect` makes another attempt; and each host's attempt count.
fn counted(
    names: &[&str],
) -> (
    LiveHosts,
    BTreeMap<String, Arc<AtomicU32>>,
    mpsc::UnboundedReceiver<StoreUpdate>,
) {
    let counts: BTreeMap<String, Arc<AtomicU32>> = names
        .iter()
        .map(|name| ((*name).to_owned(), Arc::default()))
        .collect();
    let shared = counts.clone();
    let start: Start = Arc::new(move |name: &str| {
        let count = Arc::clone(&shared[name]);
        let hour = Duration::from_secs(3600);
        LiveHost::start(Unreachable(count), Backoff::new(hour, hour))
    });
    let (updates, received) = mpsc::unbounded_channel();
    (LiveHosts::with_start(start, updates), counts, received)
}

async fn until_attempts(count: &AtomicU32, attempts: u32) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while count.load(Ordering::SeqCst) < attempts {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the attempts run");
}

#[tokio::test]
async fn reconnect_tries_only_the_host_it_names() {
    let (live, counts, _updates) = counted(&["a", "b"]);
    live.follow(["a".to_owned(), "b".to_owned()]);
    until_attempts(&counts["a"], 1).await;
    until_attempts(&counts["b"], 1).await;

    assert!(live.reconnect("a"));
    until_attempts(&counts["a"], 2).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(counts["b"].load(Ordering::SeqCst), 1, "b is left alone");
    assert!(!live.reconnect("gone"), "a host with no link is not tried");
}

#[tokio::test]
async fn a_host_no_longer_listed_has_its_link_stopped() {
    let (live, counts, _updates) = counted(&["a", "b"]);
    live.follow(["a".to_owned(), "b".to_owned()]);
    until_attempts(&counts["a"], 1).await;
    live.follow(["b".to_owned()]);
    assert!(!live.reconnect("a"), "the link of a is gone");
    live.stop("b");
    assert!(!live.reconnect("b"), "the link of b is gone");
}

#[tokio::test]
async fn each_state_reaches_the_store_under_its_host() {
    let (live, _counts, mut updates) = counted(&["a"]);
    live.follow(["a".to_owned()]);
    let update = tokio::time::timeout(Duration::from_secs(10), updates.recv())
        .await
        .expect("a state is sent")
        .expect("the channel is open");
    assert!(
        matches!(
            &update,
            StoreUpdate::HostLinkChanged { name, link: HostLink::Reconnecting { attempt: 1, reason, .. } }
                if name == "a" && reason.starts_with("The host is unreachable")
        ),
        "{update:?}"
    );
}

/// Ends on its first attempt, as a host that does not know this device; every later attempt finds it unreachable.
#[derive(Clone, Default)]
struct EndsFirst(Arc<AtomicU32>);

impl Connect for EndsFirst {
    async fn connect(&mut self) -> Result<Client, ClientError> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(ClientError::HostNotFound("a".to_owned()));
        }
        Err(ClientError::HostUnreachable("refused".to_owned()))
    }
}

async fn next_link(updates: &mut mpsc::UnboundedReceiver<StoreUpdate>) -> HostLink {
    let update = tokio::time::timeout(Duration::from_secs(10), updates.recv())
        .await
        .expect("a state is sent")
        .expect("the channel is open");
    match update {
        StoreUpdate::HostLinkChanged { link, .. } => link,
        other => panic!("a link change was expected: {other:?}"),
    }
}

#[tokio::test]
async fn an_ended_link_is_tried_again_when_its_host_is_announced_again() {
    let hour = Duration::from_secs(3600);
    let start: Start =
        Arc::new(move |_: &str| LiveHost::start(EndsFirst::default(), Backoff::new(hour, hour)));
    let (sender, mut updates) = mpsc::unbounded_channel();
    let live = LiveHosts::with_start(start, sender);
    live.follow(["a".to_owned()]);
    let ended = next_link(&mut updates).await;
    assert!(matches!(ended, HostLink::Failed(_)), "{ended:?}");

    // An SSH re-add or a new pairing of the same host: the link that ended is the one that tries again.
    live.follow_again(["a".to_owned()], "a");
    // A short-lived "Checking" may be folded into the state after it.
    let mut again = next_link(&mut updates).await;
    if again == HostLink::Checking {
        again = next_link(&mut updates).await;
    }
    assert!(
        matches!(again, HostLink::Reconnecting { attempt: 1, .. }),
        "{again:?}"
    );
}

#[tokio::test]
async fn a_borrower_learns_that_a_stopped_link_is_gone() {
    let (live, counts, _updates) = counted(&["a"]);
    live.follow(["a".to_owned()]);
    until_attempts(&counts["a"], 1).await;
    let mut watch = live.watch("a").expect("a has a link");
    let unreached = live.client("a").await.expect_err("a is out of reach");
    assert!(
        matches!(*unreached, ClientError::HostUnreachable(_)),
        "{unreached:?}"
    );

    live.stop("a");
    let ended = tokio::time::timeout(Duration::from_secs(10), watch.changed())
        .await
        .expect("the borrower hears of it");
    assert!(!ended, "the link has stopped");
    let gone = live.client("a").await.expect_err("a has no link");
    assert!(matches!(*gone, ClientError::HostNotFound(_)), "{gone:?}");
}
