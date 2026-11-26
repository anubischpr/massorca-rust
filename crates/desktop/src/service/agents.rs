//! The agent sessions the bridge reads and writes (decisions 0050 to 0057). The list comes on each connect and
//! each `agent.changed` event refetches the session it names, so the store follows the service without
//! optimistic updates. A bring-back refetches the tree it touched, as git refetches a workspace after a write.

use std::future::Future;

use massorca_client::{Client, ClientError};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    AgentBringBackParams, AgentDelegateParams, AgentDelegated, AgentKind, AgentMarkSeenParams,
    AgentModeSet, AgentPromptParams, AgentResumeParams, AgentSession, AgentSetModeParams,
    AgentStartParams, BringBackResult, PromptReceipt,
};
use tokio::sync::{mpsc, oneshot};

use super::agent_forget::{self, ForgetSessionParams};
use super::bridge::{Command, ServiceHandle};
use super::refusal::Refusal;
use super::update::StoreUpdate;

/// Lists every agent session, archived ones too when the service offers `agent-archive/1` (the Sidebar hides them until
/// Show archived). A service that predates agents answers with a remote error, which lists none; a lost link is passed
/// on so the session ends and reconnects.
pub(super) async fn list_sessions(client: &Client) -> Result<Vec<AgentSession>, ClientError> {
    let params = massorca_protocol::methods::agents::AgentListParams {
        workspace: None,
        include_archived: client.has_capability(agent_forget::AGENT_ARCHIVE),
    };
    match client.agent_list_with(&params).await {
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

    /// `agent.mark_seen`: marks the activity named by `params.at_ms` as seen. The store changes through the
    /// service's `agent.changed` event, not from this answer.
    pub async fn agent_mark_seen(
        &self,
        params: AgentMarkSeenParams,
    ) -> Result<AgentSession, Refusal> {
        self.ask(|reply| Command::MarkSeen { params, reply }).await
    }

    /// `agent.delegate`: starts a child of a session. The store lists the child after the refetch that follows,
    /// not from this answer.
    pub async fn agent_delegate(
        &self,
        params: AgentDelegateParams,
    ) -> Result<AgentDelegated, Refusal> {
        self.ask(|reply| Command::Delegate { params, reply }).await
    }

    /// `agent.prompt`: types `params.text` into the session's terminal and submits it. The receipt says the bytes
    /// were written, not that the agent read them.
    pub async fn agent_prompt(&self, params: AgentPromptParams) -> Result<PromptReceipt, Refusal> {
        self.ask(|reply| Command::AgentPrompt { params, reply })
            .await
    }

    /// `agent.stop` without force: ends the session's run. The store takes the session from the answer.
    pub async fn agent_stop(&self, session: &str) -> Result<AgentSession, Refusal> {
        self.ask(|reply| Command::AgentStop {
            session: session.to_owned(),
            reply,
        })
        .await
    }

    /// `agent.forget` with the `agent-archive/1` options: forgets a session, and with `remove_workspace` its worktree
    /// and branch too. The store follows the service's own answer and refetch.
    pub async fn agent_forget(&self, params: ForgetSessionParams) -> Result<AgentSession, Refusal> {
        self.ask(|reply| Command::AgentForget { params, reply })
            .await
    }

    /// `agent.rename` (capability `agent-archive/1`): sets a session's title. The store takes the session from the answer.
    pub async fn agent_rename(&self, session: &str, title: &str) -> Result<AgentSession, Refusal> {
        self.ask(|reply| Command::AgentRename {
            session: session.to_owned(),
            title: title.to_owned(),
            reply,
        })
        .await
    }

    /// `agent.archive_session` (capability `agent-archive/1`): archives an exited session, or clears its archive.
    pub async fn agent_archive(
        &self,
        session: &str,
        archived: bool,
    ) -> Result<AgentSession, Refusal> {
        self.ask(|reply| Command::AgentArchive {
            session: session.to_owned(),
            archived,
            reply,
        })
        .await
    }

    /// `agent.set_mode`: switches the session's permission mode now or at its next resume. The store takes the
    /// session from the answer.
    pub async fn agent_set_mode(
        &self,
        params: AgentSetModeParams,
    ) -> Result<AgentModeSet, Refusal> {
        self.ask(|reply| Command::AgentSetMode { params, reply })
            .await
    }

    /// `agent.resume`: starts a new run of an exited session, in `permission_mode` when it names one, its terminal at
    /// `grid` (cols, rows) when given. The store takes the session from the answer.
    pub async fn agent_resume(
        &self,
        session: &str,
        permission_mode: Option<String>,
        grid: Option<(u16, u16)>,
    ) -> Result<AgentSession, Refusal> {
        self.ask(|reply| Command::AgentResume {
            params: AgentResumeParams {
                session: session.to_owned(),
                permission_mode,
                cols: grid.map(|(cols, _)| cols),
                rows: grid.map(|(_, rows)| rows),
            },
            reply,
        })
        .await
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
            let _ = updates.send(StoreUpdate::AgentChanged(Box::new(session.clone())));
        }
        let _ = reply.send(result);
    });
}

/// Starts a child on its own task, then refetches the tree it joined whatever the answer, so the store lists
/// the service's own record of the child and never a guess.
pub(super) fn spawn_delegate(
    client: &Client,
    params: AgentDelegateParams,
    reply: oneshot::Sender<Result<AgentDelegated, Refusal>>,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    let client = client.clone();
    let updates = updates.clone();
    tokio::spawn(async move {
        let answer = client
            .agent_delegate(&params)
            .await
            .map_err(|error| Refusal::from(&error));
        let tree_of = match &answer {
            Ok(delegated) => delegated.session.id.clone(),
            Err(_) => params.parent.clone(),
        };
        refetch_tree(&client, &tree_of, &updates).await;
        let _ = reply.send(answer);
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

/// Marks a session's activity seen on its own task. Nothing is refetched: the service announces the change.
pub(super) fn spawn_mark_seen(
    client: &Client,
    params: AgentMarkSeenParams,
    reply: oneshot::Sender<Result<AgentSession, Refusal>>,
) {
    let client = client.clone();
    tokio::spawn(async move {
        let result = client
            .agent_mark_seen(&params.session, params.at_ms)
            .await
            .map_err(|error| Refusal::from(&error));
        let _ = reply.send(result);
    });
}

/// Types a prompt into a session on its own task. Nothing changes in the store: the session's state follows the
/// service's own reports.
pub(super) fn spawn_prompt(
    client: &Client,
    params: AgentPromptParams,
    reply: oneshot::Sender<Result<PromptReceipt, Refusal>>,
) {
    let client = client.clone();
    tokio::spawn(async move {
        let result = client
            .agent_prompt(&params)
            .await
            .map_err(|error| Refusal::from(&error));
        let _ = reply.send(result);
    });
}

/// Renames a session on its own task. The store takes the session from the service's answer.
pub(super) fn spawn_rename(
    client: &Client,
    session: String,
    title: String,
    reply: oneshot::Sender<Result<AgentSession, Refusal>>,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    let client = client.clone();
    spawn_session_answer(updates, reply, async move {
        client.agent_rename(&session, &title).await
    });
}

/// Archives or unarchives a session on its own task. The store takes the session from the service's answer.
pub(super) fn spawn_archive(
    client: &Client,
    session: String,
    archived: bool,
    reply: oneshot::Sender<Result<AgentSession, Refusal>>,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    let client = client.clone();
    spawn_session_answer(updates, reply, async move {
        client.agent_archive_session(&session, archived).await
    });
}

/// Stops a session's run on its own task, without force.
pub(super) fn spawn_stop(
    client: &Client,
    session: String,
    reply: oneshot::Sender<Result<AgentSession, Refusal>>,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    let client = client.clone();
    spawn_session_answer(updates, reply, async move {
        client.agent_stop(&session, false).await
    });
}

/// Starts a new run of an exited session on its own task.
pub(super) fn spawn_resume(
    client: &Client,
    params: AgentResumeParams,
    reply: oneshot::Sender<Result<AgentSession, Refusal>>,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    let client = client.clone();
    spawn_session_answer(updates, reply, async move {
        client.agent_resume_with(&params).await
    });
}

/// Forgets a session on its own task, then reads its tree again whatever the answer: a forgotten session leaves
/// the store, and a refused one is read as the service has it. Without the capability nothing is sent.
pub(super) fn spawn_forget(
    client: &Client,
    params: ForgetSessionParams,
    reply: oneshot::Sender<Result<AgentSession, Refusal>>,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    let client = client.clone();
    let updates = updates.clone();
    tokio::spawn(async move {
        if !client.has_capability(agent_forget::AGENT_ARCHIVE) {
            let _ = reply.send(Err(Refusal::Failed {
                code: "service.capability_missing".to_owned(),
                message: "This service cannot forget a session with these options yet.".to_owned(),
            }));
            return;
        }
        let result = agent_forget::forget(&client, &params)
            .await
            .map_err(|error| Refusal::from(&error));
        refetch_tree(&client, &params.session, &updates).await;
        let _ = reply.send(result);
    });
}

/// Sets a session's mode on its own task. The store hears of the session before the caller.
pub(super) fn spawn_set_mode(
    client: &Client,
    params: AgentSetModeParams,
    reply: oneshot::Sender<Result<AgentModeSet, Refusal>>,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    let client = client.clone();
    let updates = updates.clone();
    tokio::spawn(async move {
        let result = client
            .agent_set_mode(&params)
            .await
            .map_err(|error| Refusal::from(&error));
        if let Ok(set) = &result {
            let _ = updates.send(StoreUpdate::AgentChanged(Box::new(set.session.clone())));
        }
        let _ = reply.send(result);
    });
}

/// Awaits a call that answers with a session on its own task. The store hears of the session before the caller.
fn spawn_session_answer(
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    reply: oneshot::Sender<Result<AgentSession, Refusal>>,
    call: impl Future<Output = Result<AgentSession, ClientError>> + Send + 'static,
) {
    let updates = updates.clone();
    tokio::spawn(async move {
        let result = call.await.map_err(|error| Refusal::from(&error));
        if let Ok(session) = &result {
            let _ = updates.send(StoreUpdate::AgentChanged(Box::new(session.clone())));
        }
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
                let _ = updates.send(StoreUpdate::AgentChanged(Box::new(member)));
            }
        }
        Err(ClientError::Remote(body)) if body.code == codes::AGENT_NOT_FOUND => {
            let _ = updates.send(StoreUpdate::AgentGone(session.to_owned()));
        }
        Err(_) => {}
    }
}
