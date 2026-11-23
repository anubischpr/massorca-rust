//! The agent sessions the bridge reads and writes (decisions 0050 to 0057). The list comes on each connect and
//! each `agent.changed` event refetches the session it names, so the store follows the service without
//! optimistic updates. A bring-back refetches the tree it touched, as git refetches a workspace after a write.

use massorca_client::{Client, ClientError};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    AgentBringBackParams, AgentKind, AgentSession, AgentStartParams, BringBackResult,
};
use tokio::sync::{mpsc, oneshot};

use super::bridge::{Command, ServiceHandle};
use super::refusal::Refusal;
use super::update::StoreUpdate;

/// Lists every agent session. A service that predates agents answers with a remote error, which lists
/// none; a lost link is passed on so the session ends and reconnects.
pub(super) async fn list_sessions(client: &Client) -> Result<Vec<AgentSession>, ClientError> {
    match client.agent_list(None).await {
        Ok(list) => Ok(list.sessions),
        Err(ClientError::Remote(_)) => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

/// Lists every agent kind the service declares. A service that predates `agent.kinds` lists none, as
/// `list_sessions` does for agents; a lost link is passed on.
pub(super) async fn list_kinds(client: &Client) -> Result<Vec<AgentKind>, ClientError> {
    match client.agent_kinds().await {
        Ok(listed) => Ok(listed.kinds),
        Err(ClientError::Remote(_)) => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

impl ServiceHandle {
    /// `agent.start`: starts a session in a workspace. The store takes it from the answer, so the answer
    /// is the only place the new session comes from.
    pub async fn agent_start(&self, params: AgentStartParams) -> Result<AgentSession, Refusal> {
        self.ask(|reply| Command::AgentStart { params, reply })
            .await
    }

    /// `agent.bring_back`: merges a finished child into its parent and tells the parent run about it. The
    /// store changes through the refetch that follows, not from this answer.
    pub async fn agent_bring_back(
        &self,
        params: AgentBringBackParams,
    ) -> Result<BringBackResult, Refusal> {
        self.ask(|reply| Command::BringBack { params, reply }).await
    }
}

/// Starts a session on its own task. The store hears of the session before the caller does, so the caller
/// never opens a session the store does not list yet.
pub(super) fn spawn_start(
    client: &Client,
    params: AgentStartParams,
    reply: oneshot::Sender<Result<AgentSession, Refusal>>,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    let client = client.clone();
    let updates = updates.clone();
    tokio::spawn(async move {
        let result = client
            .agent_start(&params)
            .await
            .map_err(|error| Refusal::from(&error));
        if let Ok(session) = &result {
            let _ = updates.send(StoreUpdate::AgentChanged(session.clone()));
        }
        let _ = reply.send(result);
    });
}

/// Runs a bring-back on its own task, then refetches the tree the child belongs to whatever the answer, so the
/// store follows the merge and the parent's prompt.
pub(super) fn spawn_bring_back(
    client: &Client,
    params: AgentBringBackParams,
    reply: oneshot::Sender<Result<BringBackResult, Refusal>>,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    let client = client.clone();
    let updates = updates.clone();
    tokio::spawn(async move {
        let result = client
            .agent_bring_back(&params)
            .await
            .map_err(|error| Refusal::from(&error));
        refetch_tree(&client, &params.child, &updates).await;
        let _ = reply.send(result);
    });
}

/// Folds in every session of the tree `session` belongs to. A session the service no longer has leaves the
/// store; any other failure keeps what the store holds, and the next event or connect corrects it.
async fn refetch_tree(
    client: &Client,
    session: &str,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    match client.agent_tree(session).await {
        Ok(tree) => {
            for member in std::iter::once(tree.root).chain(tree.children) {
                let _ = updates.send(StoreUpdate::AgentChanged(member));
            }
        }
        Err(ClientError::Remote(body)) if body.code == codes::AGENT_NOT_FOUND => {
            let _ = updates.send(StoreUpdate::AgentGone(session.to_owned()));
        }
        Err(_) => {}
    }
}
