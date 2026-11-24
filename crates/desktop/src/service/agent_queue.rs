//! Each agent session's prompt queue as the bridge reads and writes it (decision 0084, capability `agent-queue/1`).
//! Every session's queue is read on each connect, and each `agent.queue_changed` event reads the session it names
//! again. A write only answers its caller: the store follows the event the service sends for it.

use massorca_client::{Client, ClientError};
use massorca_protocol::capabilities;
use massorca_protocol::methods::agents::{
    AgentQueueEditParams, AgentQueueList, AgentQueueParams, AgentSession, QueuedPrompt,
};
use tokio::sync::oneshot;

use super::bridge::{Command, ServiceHandle};
use super::refusal::Refusal;
use super::update::StoreUpdate;

/// A queue call from the UI, answered on its oneshot.
#[derive(Debug)]
pub(crate) enum QueueCommand {
    Add {
        params: AgentQueueParams,
        reply: oneshot::Sender<Result<QueuedPrompt, Refusal>>,
    },
    List {
        session: String,
        reply: oneshot::Sender<Result<AgentQueueList, Refusal>>,
    },
    Edit {
        params: AgentQueueEditParams,
        reply: oneshot::Sender<Result<QueuedPrompt, Refusal>>,
    },
    Remove {
        id: String,
        reply: oneshot::Sender<Result<QueuedPrompt, Refusal>>,
    },
    SendNow {
        id: String,
        reply: oneshot::Sender<Result<QueuedPrompt, Refusal>>,
    },
    Resume {
        session: String,
        reply: oneshot::Sender<Result<AgentQueueList, Refusal>>,
    },
}

impl ServiceHandle {
    /// `agent.queue`: queues a prompt behind the session's busy agent.
    pub async fn agent_queue(&self, params: AgentQueueParams) -> Result<QueuedPrompt, Refusal> {
        self.ask(|reply| Command::AgentQueue(QueueCommand::Add { params, reply }))
            .await
    }

    /// `agent.queue.list`: the session's queue, head first.
    pub async fn agent_queue_list(&self, session: &str) -> Result<AgentQueueList, Refusal> {
        let session = session.to_owned();
        self.ask(|reply| Command::AgentQueue(QueueCommand::List { session, reply }))
            .await
    }

    /// `agent.queue.edit`: replaces the text of a waiting or failed prompt; it keeps its place.
    pub async fn agent_queue_edit(
        &self,
        params: AgentQueueEditParams,
    ) -> Result<QueuedPrompt, Refusal> {
        self.ask(|reply| Command::AgentQueue(QueueCommand::Edit { params, reply }))
            .await
    }

    /// `agent.queue.remove`: removes a waiting or failed prompt.
    pub async fn agent_queue_remove(&self, id: &str) -> Result<QueuedPrompt, Refusal> {
        let id = id.to_owned();
        self.ask(|reply| Command::AgentQueue(QueueCommand::Remove { id, reply }))
            .await
    }

    /// `agent.queue.send_now`: types one prompt at once, ahead of the queue; it leaves the queue.
    pub async fn agent_queue_send_now(&self, id: &str) -> Result<QueuedPrompt, Refusal> {
        let id = id.to_owned();
        self.ask(|reply| Command::AgentQueue(QueueCommand::SendNow { id, reply }))
            .await
    }

    /// `agent.queue.resume`: lifts the session's queue pauses.
    pub async fn agent_queue_resume(&self, session: &str) -> Result<AgentQueueList, Refusal> {
        let session = session.to_owned();
        self.ask(|reply| Command::AgentQueue(QueueCommand::Resume { session, reply }))
            .await
    }
}

/// Reads the queue of every session. `None` when the service offers no queue; a session whose list is refused holds
/// none. A lost link is passed on.
pub(super) async fn read_all(
    client: &Client,
    sessions: &[AgentSession],
) -> Result<Option<Vec<AgentQueueList>>, ClientError> {
    if !client.has_capability(capabilities::AGENT_QUEUE) {
        return Ok(None);
    }
    let mut queues = Vec::new();
    for session in sessions {
        match client.agent_queue_list(&session.id).await {
            Ok(list) => queues.push(list),
            Err(ClientError::Remote(_)) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(Some(queues))
}

/// Reads one session's queue again after its `agent.queue_changed` event. A refused list empties it.
pub(super) async fn read_one(client: &Client, session: &str) -> Result<StoreUpdate, ClientError> {
    match client.agent_queue_list(session).await {
        Ok(list) => Ok(StoreUpdate::AgentQueueChanged(list)),
        Err(ClientError::Remote(_)) => Ok(StoreUpdate::AgentQueueChanged(AgentQueueList {
            session: session.to_owned(),
            items: Vec::new(),
        })),
        Err(error) => Err(error),
    }
}

/// Runs a queue call on its own task.
pub(super) fn spawn_call(client: &Client, command: QueueCommand) {
    let client = client.clone();
    tokio::spawn(async move {
        match command {
            QueueCommand::Add { params, reply } => answer(reply, client.agent_queue(&params).await),
            QueueCommand::List { session, reply } => {
                answer(reply, client.agent_queue_list(&session).await);
            }
            QueueCommand::Edit { params, reply } => {
                answer(reply, client.agent_queue_edit(&params).await);
            }
            QueueCommand::Remove { id, reply } => {
                answer(reply, client.agent_queue_remove(&id).await);
            }
            QueueCommand::SendNow { id, reply } => {
                answer(reply, client.agent_queue_send_now(&id).await);
            }
            QueueCommand::Resume { session, reply } => {
                answer(reply, client.agent_queue_resume(&session).await);
            }
        }
    });
}

fn answer<T>(reply: oneshot::Sender<Result<T, Refusal>>, result: Result<T, ClientError>) {
    let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
}

/// While there is no connection a queue call cannot run, and says so at once.
pub(super) fn refuse_while_down(command: QueueCommand, refusal: Refusal) {
    match command {
        QueueCommand::Add { reply, .. }
        | QueueCommand::Edit { reply, .. }
        | QueueCommand::Remove { reply, .. }
        | QueueCommand::SendNow { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        QueueCommand::List { reply, .. } | QueueCommand::Resume { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
    }
}
