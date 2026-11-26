//! Chat sessions as the bridge serves them (decision 0101, capability `agent-chat/1`): the calls that start one, answer
//! its approvals, interrupt its turn and page its history, and the stream of its transcript. The stream follows one
//! session for as long as the UI shows it and starts again after a lost link from the cursor the bridge last applied.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use massorca_client::{Client, ClientError};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{AgentSession, AgentStartParams};
use massorca_protocol::methods::chat::{
    self, ChatApprovalAnswerParams, ChatEvent, ChatHistoryPage, ChatHistoryParams, ChatInterrupted,
    ChatItem, ChatSubscribeParams, EndReason,
};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use super::bridge::{Command, ServiceHandle};
use super::git::WatchRestarts;
use super::refusal::Refusal;
use super::update::StoreUpdate;
use crate::model::chat_transcript::{ChatTranscript, Next};

/// A chat call from the UI, answered on its oneshot; or a change of the session whose stream the bridge follows.
#[derive(Debug)]
pub(crate) enum ChatCommand {
    Start {
        params: AgentStartParams,
        reply: oneshot::Sender<Result<AgentSession, Refusal>>,
    },
    Answer {
        params: ChatApprovalAnswerParams,
        reply: oneshot::Sender<Result<ChatItem, Refusal>>,
    },
    Interrupt {
        session: String,
        reply: oneshot::Sender<Result<ChatInterrupted, Refusal>>,
    },
    History {
        params: ChatHistoryParams,
        reply: oneshot::Sender<Result<ChatHistoryPage, Refusal>>,
    },
    /// Follows the session's transcript until `Unwatch`, across reconnects.
    Watch {
        session: String,
    },
    Unwatch {
        session: String,
    },
}

/// A session the start page started, and why it is not the kind of session the person chose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartedSession {
    pub session: AgentSession,
    /// Chat was asked for and refused, so a terminal session started instead; the sentence says why.
    pub fell_back: Option<String>,
}

impl ServiceHandle {
    /// Starts a session from the start page: as a chat when `chat` is set (no grid, since a chat has no terminal), else
    /// in a terminal. A chat the service cannot run (`agent.chat_unsupported`, or no `agent-chat/1`) starts as a terminal
    /// session instead, with the reason kept (decision 0101, D1).
    pub async fn start_session(
        &self,
        params: AgentStartParams,
        chat: bool,
    ) -> Result<StartedSession, Refusal> {
        if chat {
            let chat_params = AgentStartParams {
                cols: None,
                rows: None,
                ..params.clone()
            };
            match self.agent_chat_start(chat_params).await {
                Ok(session) => {
                    return Ok(StartedSession {
                        session,
                        fell_back: None,
                    });
                }
                Err(refusal) if cannot_chat(&refusal) => {
                    let session = self.agent_start(params).await?;
                    return Ok(StartedSession {
                        session,
                        fell_back: Some(refusal.sentence()),
                    });
                }
                Err(refusal) => return Err(refusal),
            }
        }
        let session = self.agent_start(params).await?;
        Ok(StartedSession {
            session,
            fell_back: None,
        })
    }

    /// `agent.chat.start`: starts a session whose run is a chat, with the first prompt in the same call. The store takes
    /// the session from the answer. Refused with `agent.chat_unsupported` when the kind cannot chat.
    pub async fn agent_chat_start(
        &self,
        params: AgentStartParams,
    ) -> Result<AgentSession, Refusal> {
        self.ask(|reply| Command::Chat(ChatCommand::Start { params, reply }))
            .await
    }

    /// `agent.chat.approval.answer`: answers one permission request of a chat session. The answer is the approval item
    /// after it; the stream carries the same item.
    pub async fn agent_chat_answer(
        &self,
        params: ChatApprovalAnswerParams,
    ) -> Result<ChatItem, Refusal> {
        self.ask(|reply| Command::Chat(ChatCommand::Answer { params, reply }))
            .await
    }

    /// `agent.chat.interrupt`: ends the running turn and keeps the session.
    pub async fn agent_chat_interrupt(&self, session: &str) -> Result<ChatInterrupted, Refusal> {
        let session = session.to_owned();
        self.ask(|reply| Command::Chat(ChatCommand::Interrupt { session, reply }))
            .await
    }

    /// `agent.chat.history`: the page of items before `params.before`. The store adds the page to the transcript too.
    pub async fn agent_chat_history(
        &self,
        params: ChatHistoryParams,
    ) -> Result<ChatHistoryPage, Refusal> {
        self.ask(|reply| Command::Chat(ChatCommand::History { params, reply }))
            .await
    }

    /// Starts following `session`'s transcript: the store holds it from the stream's snapshot on.
    pub fn watch_chat(&self, session: &str) {
        self.send(Command::Chat(ChatCommand::Watch {
            session: session.to_owned(),
        }));
    }

    /// Stops following `session` and drops its transcript from the store.
    pub fn unwatch_chat(&self, session: &str) {
        self.send(Command::Chat(ChatCommand::Unwatch {
            session: session.to_owned(),
        }));
    }
}

/// Whether a refused chat start leaves a terminal start possible: the kind cannot chat, or the service cannot at all.
fn cannot_chat(refusal: &Refusal) -> bool {
    matches!(
        refusal.code(),
        Some(codes::AGENT_CHAT_UNSUPPORTED | codes::SERVICE_CAPABILITY_MISSING)
    )
}

/// Each followed session's transcript as the bridge applied it, which gives the cursor a new stream starts after and
/// tells a gap in an append. It outlives the connection, so a reconnect continues where the stream left off.
#[derive(Clone, Debug, Default)]
pub(super) struct ChatMirrors(Arc<Mutex<HashMap<String, ChatTranscript>>>);

impl ChatMirrors {
    fn apply(&self, session: &str, event: ChatEvent) -> Next {
        let mut held = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        held.entry(session.to_owned()).or_default().apply(event)
    }

    fn cursor(&self, session: &str) -> Option<chat::ChatCursor> {
        let held = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        held.get(session)?.cursor().cloned()
    }

    pub(super) fn forget(&self, session: &str) {
        let mut held = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        held.remove(session);
    }
}

/// The follow tasks of one connection, one per followed session. Dropping the set aborts them.
#[derive(Debug, Default)]
pub(super) struct ChatWatches {
    tasks: HashMap<String, JoinHandle<()>>,
}

impl ChatWatches {
    pub(super) fn start(
        &mut self,
        client: &Client,
        session: &str,
        mirrors: &ChatMirrors,
        updates: &mpsc::UnboundedSender<StoreUpdate>,
    ) {
        if self
            .tasks
            .get(session)
            .is_some_and(|task| !task.is_finished())
        {
            return;
        }
        let task = tokio::spawn(follow(
            client.clone(),
            session.to_owned(),
            mirrors.clone(),
            updates.clone(),
        ));
        self.tasks.insert(session.to_owned(), task);
    }

    pub(super) fn stop(&mut self, session: &str) {
        if let Some(task) = self.tasks.remove(session) {
            task.abort();
        }
    }
}

impl Drop for ChatWatches {
    fn drop(&mut self) {
        for (_, task) in self.tasks.drain() {
            task.abort();
        }
    }
}

/// Stops following `session`: the bridge forgets it, and the store drops its transcript.
pub(super) fn note_unwatch(
    session: &str,
    watching: &mut BTreeSet<String>,
    mirrors: &ChatMirrors,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    watching.remove(session);
    mirrors.forget(session);
    let _ = updates.send(StoreUpdate::ChatClosed(session.to_owned()));
}

/// Follows one session until its stream ends for good: the session was forgotten, the service refused the stream, or the
/// restart budget of a lost stream is spent. A lost link ends the task with its connection; the next connection starts
/// it again from the mirror's cursor.
async fn follow(
    client: Client,
    session: String,
    mirrors: ChatMirrors,
    updates: mpsc::UnboundedSender<StoreUpdate>,
) {
    let mut restarts = WatchRestarts::new();
    loop {
        let params = ChatSubscribeParams {
            session: session.clone(),
            after: mirrors.cursor(&session),
            limit: None,
        };
        let ending = match client.subscribe(chat::SUBSCRIBE, &params).await {
            Ok(mut events) => serve(&mut events, &session, &mirrors, &updates).await,
            Err(ClientError::Remote(body)) => {
                // Not a chat, or not there: nothing a restart would change.
                let refusal = Refusal::from(&ClientError::Remote(body));
                let _ = updates.send(StoreUpdate::ChatRefused { session, refusal });
                return;
            }
            Err(_) => Ending::Lost { healthy: false },
        };
        match ending {
            Ending::Over => return,
            Ending::Again => {}
            Ending::Lost { healthy } => {
                if healthy {
                    restarts.healthy();
                }
                let Some(pause) = restarts.lost() else {
                    return;
                };
                tokio::time::sleep(pause).await;
            }
        }
    }
}

enum Ending {
    /// The service ended the stream: the session is gone.
    Over,
    /// The stream asks to be opened again at once, after the cursor.
    Again,
    /// The stream broke; `healthy` when it had delivered an event.
    Lost { healthy: bool },
}

async fn serve(
    events: &mut massorca_client::Subscription,
    session: &str,
    mirrors: &ChatMirrors,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) -> Ending {
    let mut healthy = false;
    loop {
        match events.next_as::<ChatEvent>().await {
            Some(Ok(event)) => {
                healthy = true;
                let ended = matches!(
                    event,
                    ChatEvent::End {
                        reason: EndReason::Forgotten | EndReason::ShuttingDown | EndReason::Unknown
                    }
                );
                let next = mirrors.apply(session, event.clone());
                let _ = updates.send(StoreUpdate::Chat {
                    session: session.to_owned(),
                    event,
                });
                if ended {
                    return Ending::Over;
                }
                if next == Next::Resubscribe {
                    return Ending::Again;
                }
            }
            // An event this build cannot read changes nothing it shows (decision 0002).
            Some(Err(ClientError::Protocol(_))) => {}
            Some(Err(ClientError::Remote(body))) if body.code == codes::STREAM_OVERFLOW => {
                return Ending::Again;
            }
            Some(Err(_)) | None => return Ending::Lost { healthy },
        }
    }
}

/// Runs a chat call on its own task, so a slow one never holds up the connection's events.
pub(super) fn spawn_call(
    client: &Client,
    command: ChatCommand,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    let client = client.clone();
    let updates = updates.clone();
    tokio::spawn(async move {
        match command {
            ChatCommand::Start { params, reply } => {
                let result = client.agent_chat_start(&params).await;
                // The store hears of the session before the caller does, as it does for `agent.start`.
                if let Ok(session) = &result {
                    let _ = updates.send(StoreUpdate::AgentChanged(Box::new(session.clone())));
                }
                answer(reply, result);
            }
            ChatCommand::Answer { params, reply } => {
                answer(reply, client.agent_chat_answer(&params).await);
            }
            ChatCommand::Interrupt { session, reply } => {
                answer(reply, client.agent_chat_interrupt(&session).await);
            }
            ChatCommand::History { params, reply } => {
                let session = params.session.clone();
                let result: Result<ChatHistoryPage, _> = client.call(chat::HISTORY, &params).await;
                if let Ok(page) = &result {
                    let _ = updates.send(StoreUpdate::ChatHistory {
                        session,
                        page: page.clone(),
                    });
                }
                answer(reply, result);
            }
            ChatCommand::Watch { .. } | ChatCommand::Unwatch { .. } => {}
        }
    });
}

fn answer<T>(reply: oneshot::Sender<Result<T, Refusal>>, result: Result<T, ClientError>) {
    let _ = reply.send(result.map_err(|error| Refusal::from(&error)));
}

/// While there is no connection a chat call cannot run, and says so at once.
pub(super) fn refuse_while_down(command: ChatCommand, refusal: Refusal) {
    match command {
        ChatCommand::Start { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        ChatCommand::Answer { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        ChatCommand::Interrupt { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        ChatCommand::History { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        ChatCommand::Watch { .. } | ChatCommand::Unwatch { .. } => {}
    }
}
