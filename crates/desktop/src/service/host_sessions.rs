//! The agent sessions a paired host runs (STAT-16). Each paired host is followed by one task on its own
//! connection: it reads the host's sessions, then re-reads them on each `agent.changed` event the host sends. A
//! host whose connection cannot be made, or whose event subscription is refused, is checked again by the bridge's
//! poll. A host that stops answering keeps the sessions it last ran, unverified: losing contact is never proof
//! that a run ended.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use massorca_client::{Client, ClientError, Hosts};
use massorca_profile::Profile;
use massorca_protocol::codes;
use massorca_protocol::methods::agents::AgentSession;
use massorca_protocol::methods::events::{ChangeEvent, TOPIC_AGENT_CHANGED};
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceListParams};
use tokio::sync::{mpsc, oneshot};

use super::agents::list_sessions;
use super::bridge::ServiceHandle;
use super::extensions::connect_host;
use super::hosts::peer;
use super::refusal::Refusal;
use super::update::{HostLink, StoreUpdate};

static CHECKS: AtomicU64 = AtomicU64::new(0);

/// Numbers one check of a host. A later check always has a larger number.
fn next_check() -> u64 {
    CHECKS.fetch_add(1, Ordering::Relaxed) + 1
}

/// The paired hosts a task follows, by name. A host is followed by at most one task at a time.
#[derive(Clone, Debug, Default)]
pub(super) struct Followed(Arc<Mutex<BTreeSet<String>>>);

impl Followed {
    /// Claims `host` for one task. `None` when a task already follows it, so the poll leaves it alone.
    pub(super) fn claim(&self, host: &str) -> Option<Claim> {
        let mut held = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        held.insert(host.to_owned()).then(|| Claim {
            followed: self.clone(),
            host: host.to_owned(),
        })
    }

    fn release(&self, host: &str) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(host);
    }
}

/// A host's claim on its follower. Dropping it, even on a panic, lets the poll check the host again.
pub(super) struct Claim {
    followed: Followed,
    host: String,
}

impl Drop for Claim {
    fn drop(&mut self) {
        self.followed.release(&self.host);
    }
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

/// Follows one paired host until its connection ends. Its first read reports the host; each `agent.changed`
/// event reports it again. A connection that cannot be made, or one the host does not let subscribe, ends here:
/// the bridge's poll checks the host again, and the claim is released with the task.
pub(super) async fn follow(
    profile: Profile,
    host: String,
    claim: Claim,
    updates: mpsc::UnboundedSender<StoreUpdate>,
) {
    let _claim = claim;
    let check = next_check();
    let client = match Client::connect_host(&Hosts::of(&profile), &host, &peer()).await {
        Ok(client) => client,
        Err(error) => {
            report_unreached(&updates, &host, check);
            let _ = updates.send(StoreUpdate::HostLinkChanged {
                name: host,
                link: HostLink::from_error(&error),
            });
            return;
        }
    };
    report_read(&client, &host, check, &updates).await;
    let _ = updates.send(StoreUpdate::HostLinkChanged {
        name: host.clone(),
        link: connected(&client),
    });
    let Ok(mut events) = client.subscribe_events().await else {
        return;
    };
    loop {
        match events.next_as::<ChangeEvent>().await {
            Some(Ok(event)) if event.topic == TOPIC_AGENT_CHANGED => {
                report_read(&client, &host, check, &updates).await;
            }
            Some(Ok(_)) => {}
            Some(Err(ClientError::Remote(body))) if body.code == codes::STREAM_OVERFLOW => {
                report_read(&client, &host, check, &updates).await;
            }
            // A change this build cannot read changes nothing it shows.
            Some(Err(ClientError::Protocol(_))) => {}
            Some(Err(_)) | None => break,
        }
    }
    report_unreached(&updates, &host, check);
    let _ = updates.send(StoreUpdate::HostLinkChanged {
        name: host,
        link: HostLink::Unreachable("The connection to this host was lost.".to_owned()),
    });
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

pub(super) fn connected(client: &Client) -> HostLink {
    let server = &client.welcome().server;
    HostLink::Connected {
        server: server.name.clone(),
        version: server.version.clone(),
    }
}

/// Stops a session on its own host: `agent.stop` on the host's connection, never on This PC's service. The
/// host's follower reports the change; the answer is the host's own.
pub(super) fn spawn_stop(
    profile: Profile,
    host: String,
    session: String,
    reply: oneshot::Sender<Result<AgentSession, Refusal>>,
) {
    tokio::spawn(async move {
        let answer = match connect_host(&profile, &host).await {
            Ok(client) => client
                .agent_stop(&session, false)
                .await
                .map_err(|error| Refusal::from(&error)),
            Err(error) => Err(Refusal::from(&error)),
        };
        let _ = reply.send(answer);
    });
}

impl ServiceHandle {
    /// `agent.stop` on a paired host's own connection. The answer is the host's session.
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
    use super::{Followed, next_check};

    #[test]
    fn a_later_check_has_a_larger_number() {
        let first = next_check();
        let second = next_check();
        assert!(second > first);
    }

    #[test]
    fn a_host_is_followed_by_one_claim_at_a_time_and_released_on_drop() {
        let followed = Followed::default();
        let claim = followed.claim("vps").expect("the first claim");
        assert!(followed.claim("vps").is_none(), "a second task is refused");
        assert!(followed.claim("gpu").is_some(), "another host is free");
        drop(claim);
        assert!(
            followed.claim("vps").is_some(),
            "the poll may check it again"
        );
    }
}
