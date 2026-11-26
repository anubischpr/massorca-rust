//! The agent sessions a paired host runs (STAT-16). Each paired host is followed on its live link's connection: the
//! follower reads the host's sessions, then re-reads them on each `agent.changed` event the host sends, and starts
//! again on each connection the link makes. While the link has no connection the host keeps the sessions it last
//! ran, unverified: losing contact is never proof that a run ended.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use massorca_client::{Client, ClientError, LiveWatch};
use massorca_protocol::codes;
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
                Some(false) => {
                    report_unreached(&updates, &host, check);
                    return;
                }
                None => {}
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

/// Reads the host on one connection, then again on each `agent.changed` event, until the connection ends. A host that
/// does not let this device subscribe is read again every `READ_AGAIN_EVERY` instead.
async fn follow_connection(
    client: &Client,
    host: &str,
    check: u64,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    report_read(client, host, check, updates).await;
    let Ok(mut events) = client.subscribe_events().await else {
        loop {
            tokio::select! {
                () = client.closed() => return,
                () = tokio::time::sleep(READ_AGAIN_EVERY) => {
                    report_read(client, host, check, updates).await;
                }
            }
        }
    };
    loop {
        match events.next_as::<ChangeEvent>().await {
            Some(Ok(event)) if event.topic == TOPIC_AGENT_CHANGED => {
                report_read(client, host, check, updates).await;
            }
            Some(Ok(_)) => {}
            Some(Err(ClientError::Remote(body))) if body.code == codes::STREAM_OVERFLOW => {
                report_read(client, host, check, updates).await;
            }
            // A change this build cannot read changes nothing it shows.
            Some(Err(ClientError::Protocol(_))) => {}
            Some(Err(_)) | None => return,
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

async fn report_read(
    client: &Client,
    host: &str,
    request: u64,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    let answer = read(client).await;
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
    use super::next_check;

    #[test]
    fn a_later_check_has_a_larger_number() {
        let first = next_check();
        let second = next_check();
        assert!(second > first);
    }
}
