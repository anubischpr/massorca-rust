//! The agent sessions a paired host runs (STAT-16). Each paired host is followed on its live link's connection: the
//! follower reads the host's sessions, then re-reads them on each `agent.changed` event the host sends, and starts
//! again on each connection the link makes. While the link has no connection the host keeps the sessions it last
//! ran, unverified: losing contact is never proof that a run ended.

use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use massorca_client::{Client, ClientError, LiveWatch, Subscription};
use massorca_protocol::methods::agents::AgentSession;
use massorca_protocol::methods::events::{ChangeEvent, TOPIC_AGENT_CHANGED};
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceListParams};
use tokio::sync::{mpsc, oneshot};

use super::agents::list_sessions;
use super::bridge::ServiceHandle;
use super::live_hosts::LiveHosts;
use super::refusal::Refusal;
use super::update::StoreUpdate;

static CHECKS: AtomicU64 = AtomicU64::new(0);

/// How often a host that refuses the event subscription is read again on its connection.
const READ_AGAIN_EVERY: Duration = Duration::from_secs(30);
/// The pause before a subscription the host ended, on a connection still up, is opened again.
const SUBSCRIBE_AGAIN_AFTER: Duration = Duration::from_secs(1);

/// Numbers one check of a host. A later check always has a larger number.
fn next_check() -> u64 {
    CHECKS.fetch_add(1, Ordering::Relaxed) + 1
}

/// What a host answered for one read: its sessions, and the workspaces they run in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostAgents {
    pub sessions: Vec<AgentSession>,
    pub workspaces: Vec<Workspace>,
}

/// One read's answer about a host's sessions. `answer` is `None` when the host could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostSessionsRead {
    pub host: String,
    pub request: u64,
    pub answer: Option<HostAgents>,
}

/// The sessions a paired host last reported. `reached` says whether the newest read reached it; while it is
/// false the sessions are last known and unverifiable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostSessions {
    pub request: u64,
    pub reached: bool,
    pub sessions: Vec<AgentSession>,
    pub workspaces: Vec<Workspace>,
}

/// What the follower uses of one connection: its change events and its reads. A `Client` provides them; the tests
/// script them.
trait HostConnection: Sync {
    type Events: Send;

    fn subscribe_changes(&self) -> impl Future<Output = Result<Self::Events, ClientError>> + Send;
    /// The next change, or `None` once the subscription has ended.
    fn next_change(
        events: &mut Self::Events,
    ) -> impl Future<Output = Option<Result<ChangeEvent, ClientError>>> + Send;
    fn read_now(&self) -> impl Future<Output = Option<HostAgents>> + Send;
    /// Resolves once the connection has ended.
    fn wait_closed(&self) -> impl Future<Output = ()> + Send;
}

impl HostConnection for Client {
    type Events = Subscription;

    fn subscribe_changes(&self) -> impl Future<Output = Result<Subscription, ClientError>> + Send {
        self.subscribe_events()
    }

    async fn next_change(events: &mut Subscription) -> Option<Result<ChangeEvent, ClientError>> {
        events.next_as::<ChangeEvent>().await
    }

    async fn read_now(&self) -> Option<HostAgents> {
        read(self).await
    }

    async fn wait_closed(&self) {
        self.closed().await;
    }
}

/// Follows one paired host's sessions on its link's connection until the link stops. Each connection is read at
/// once and on each `agent.changed` event; when it is lost the sessions read so far stay, marked unreached.
pub(super) async fn follow(
    host: String,
    mut link: LiveWatch,
    updates: mpsc::UnboundedSender<StoreUpdate>,
) {
    let mut read_once = false;
    loop {
        let check = next_check();
        if let Some(client) = link.current() {
            read_once = true;
            let link_changed = tokio::select! {
                () = follow_connection(&client, &host, check, &updates) => None,
                running = link.changed() => Some(running),
            };
            drop(client);
            match link_changed {
                // The connection was lost or replaced: the loop reads what the link has now.
                Some(true) => continue,
                // The link stopped, so its host is no longer followed: nothing is reported for it.
                Some(false) => return,
                // Ended on its own: a stopped link reports nothing, a lost one is reported once the link confirms it.
                None => {
                    if !link.changed().await {
                        return;
                    }
                    continue;
                }
            }
        }
        if read_once {
            report_unreached(&updates, &host, check);
        }
        if !link.changed().await {
            return;
        }
    }
}

/// Reads the host on one connection, then again on each `agent.changed` event, until the connection ends. A
/// subscription the host ends while the connection stays up is opened again; a host that refuses subscriptions is
/// read again every `READ_AGAIN_EVERY` instead.
async fn follow_connection<C: HostConnection>(
    conn: &C,
    host: &str,
    check: u64,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    loop {
        // Subscribed before the read, so a change made between the two is still followed.
        let subscribed = conn.subscribe_changes().await;
        report_read(conn, host, check, updates).await;
        let pause = match subscribed {
            Ok(mut events) => {
                loop {
                    match C::next_change(&mut events).await {
                        Some(Ok(event)) if event.topic == TOPIC_AGENT_CHANGED => {
                            report_read(conn, host, check, updates).await;
                        }
                        // A change this build cannot read changes nothing it shows.
                        Some(Ok(_)) | Some(Err(ClientError::Protocol(_))) => {}
                        // The subscription ended: the host overflowed it, or the connection was lost.
                        Some(Err(_)) | None => break,
                    }
                }
                SUBSCRIBE_AGAIN_AFTER
            }
            Err(_) => READ_AGAIN_EVERY,
        };
        tokio::select! {
            () = conn.wait_closed() => return,
            () = tokio::time::sleep(pause) => {}
        }
    }
}

/// Reads a host's sessions and workspaces on its open connection. `None` when the sessions could not be listed.
async fn read(client: &Client) -> Option<HostAgents> {
    let (sessions, workspaces) = tokio::join!(
        list_sessions(client),
        client.workspace_list(&WorkspaceListParams { project: None })
    );
    let sessions = sessions.ok()?;
    let workspaces = workspaces
        .map(|listed| listed.workspaces)
        .unwrap_or_default();
    Some(HostAgents {
        sessions,
        workspaces,
    })
}

async fn report_read<C: HostConnection>(
    conn: &C,
    host: &str,
    request: u64,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    let answer = conn.read_now().await;
    let _ = updates.send(StoreUpdate::HostSessions(HostSessionsRead {
        host: host.to_owned(),
        request,
        answer,
    }));
}

fn report_unreached(updates: &mpsc::UnboundedSender<StoreUpdate>, host: &str, request: u64) {
    let _ = updates.send(StoreUpdate::HostSessions(HostSessionsRead {
        host: host.to_owned(),
        request,
        answer: None,
    }));
}

/// Stops a session on its own host: `agent.stop` on the host's live connection, never on This PC's service. The
/// host's follower reports the change; the answer is the host's own.
pub(super) fn spawn_stop(
    live: LiveHosts,
    host: String,
    session: String,
    reply: oneshot::Sender<Result<AgentSession, Refusal>>,
) {
    tokio::spawn(async move {
        let answer = match live.client(&host).await {
            Ok(client) => client
                .agent_stop(&session, false)
                .await
                .map_err(|error| Refusal::from(&error)),
            Err(error) => Err(Refusal::from(&*error)),
        };
        let _ = reply.send(answer);
    });
}

impl ServiceHandle {
    /// `agent.stop` on a paired host's live connection. The answer is the host's session.
    pub async fn host_agent_stop(
        &self,
        host: &str,
        session: &str,
    ) -> Result<AgentSession, Refusal> {
        self.ask(|reply| super::bridge::Command::HostAgentStop {
            host: host.to_owned(),
            session: session.to_owned(),
            reply,
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use massorca_protocol::{ErrorBody, codes};
    use tokio::sync::watch;

    use super::*;

    /// One scripted subscription: its items in order, where `None` ends it. A subscription whose items run out stays
    /// open until the connection ends.
    type Script = Result<VecDeque<Option<Result<ChangeEvent, ClientError>>>, ClientError>;

    /// A connection whose subscriptions are scripted, and which records the calls made on it in order.
    struct Scripted {
        subscriptions: Mutex<VecDeque<Script>>,
        calls: Mutex<Vec<&'static str>>,
        closed: watch::Sender<bool>,
    }

    /// The events of one scripted subscription.
    struct Subscribed {
        items: VecDeque<Option<Result<ChangeEvent, ClientError>>>,
        closed: watch::Receiver<bool>,
    }

    impl Scripted {
        fn new(subscriptions: Vec<Script>) -> Self {
            Self {
                subscriptions: Mutex::new(subscriptions.into()),
                calls: Mutex::new(Vec::new()),
                closed: watch::channel(false).0,
            }
        }

        fn calls(&self) -> Vec<&'static str> {
            self.calls.lock().expect("the calls").clone()
        }

        fn end(&self) {
            self.closed.send_replace(true);
        }
    }

    impl HostConnection for Scripted {
        type Events = Subscribed;

        async fn subscribe_changes(&self) -> Result<Subscribed, ClientError> {
            self.calls.lock().expect("the calls").push("subscribe");
            let next = self
                .subscriptions
                .lock()
                .expect("the subscriptions")
                .pop_front()
                .unwrap_or(Err(ClientError::Disconnected));
            next.map(|items| Subscribed {
                items,
                closed: self.closed.subscribe(),
            })
        }

        async fn next_change(events: &mut Subscribed) -> Option<Result<ChangeEvent, ClientError>> {
            match events.items.pop_front() {
                Some(item) => item,
                None => {
                    let _ = events.closed.wait_for(|closed| *closed).await;
                    None
                }
            }
        }

        async fn read_now(&self) -> Option<HostAgents> {
            self.calls.lock().expect("the calls").push("read");
            Some(HostAgents {
                sessions: Vec::new(),
                workspaces: Vec::new(),
            })
        }

        async fn wait_closed(&self) {
            let mut closed = self.closed.subscribe();
            let _ = closed.wait_for(|closed| *closed).await;
        }
    }

    fn agent_changed() -> Option<Result<ChangeEvent, ClientError>> {
        let event = serde_json::json!({ "topic": TOPIC_AGENT_CHANGED, "id": "s1" });
        Some(Ok(serde_json::from_value(event).expect("a change event")))
    }

    fn overflow() -> Option<Result<ChangeEvent, ClientError>> {
        Some(Err(ClientError::Remote(ErrorBody::new(
            codes::STREAM_OVERFLOW,
            "the subscriber fell behind, so the stream ended",
        ))))
    }

    fn refused() -> Script {
        Err(ClientError::Remote(ErrorBody::new(
            "method.unknown",
            "the host does not offer the subscription",
        )))
    }

    #[test]
    fn a_later_check_has_a_larger_number() {
        let first = next_check();
        let second = next_check();
        assert!(second > first);
    }

    #[tokio::test(start_paused = true)]
    async fn a_subscription_the_host_ends_on_a_live_connection_is_opened_again() {
        let connection = Arc::new(Scripted::new(vec![
            Ok(VecDeque::from([agent_changed(), overflow()])),
            Ok(VecDeque::new()),
        ]));
        let (updates, _received) = mpsc::unbounded_channel();
        let follower = tokio::spawn({
            let connection = Arc::clone(&connection);
            async move { follow_connection(&*connection, "vps", 1, &updates).await }
        });

        // The overflow ends the first subscription; the second opens after its pause and stays open.
        tokio::time::sleep(Duration::from_secs(5)).await;
        connection.end();
        follower
            .await
            .expect("the follower ends with its connection");
        assert_eq!(
            connection.calls(),
            ["subscribe", "read", "read", "subscribe", "read"],
            "subscribed before each read, and read again after the subscription came back"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_host_that_refuses_subscriptions_is_read_again_every_thirty_seconds() {
        let connection = Arc::new(Scripted::new(vec![refused(), refused()]));
        let (updates, _received) = mpsc::unbounded_channel();
        let follower = tokio::spawn({
            let connection = Arc::clone(&connection);
            async move { follow_connection(&*connection, "vps", 1, &updates).await }
        });

        tokio::time::sleep(Duration::from_secs(31)).await;
        connection.end();
        follower
            .await
            .expect("the follower ends with its connection");
        assert_eq!(
            connection.calls(),
            ["subscribe", "read", "subscribe", "read"]
        );
    }
}
