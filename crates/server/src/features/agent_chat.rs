//! The chat methods (decision 0101): `agent.chat.start` starts a session whose run is a chat, `agent.chat.subscribe`
//! streams its journal, `agent.chat.history` pages it back, `agent.chat.approval.answer` answers its permission requests
//! and `agent.chat.interrupt` ends its running turn. The rules and the journal live in `core-agents`; its journal
//! changes reach the streams through the service's chat channel, which carries `Change::Chat` and `Change::Forgotten`.
//!
//! The methods are registered under `agent-chat/1`, which the service offers now that it serves every one of them
//! (D11).

use massorca_core_agents::{AgentError, Agents, Change, ChatOpened};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{AgentSession, AgentSessionRef};
use massorca_protocol::methods::chat::{
    self, AgentChatStartParams, ChatApprovalAnswerParams, ChatCursor, ChatEvent, ChatHistoryPage,
    ChatHistoryParams, ChatInterrupted, ChatItem, ChatSubscribeParams, EndReason, ResetReason,
};
use massorca_protocol::{ErrorBody, capabilities};
use tokio::sync::{broadcast, mpsc};

use super::agent_redaction::for_caller;
use super::agents::{on_agents, require_admin_for_session, secret_grant};
use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::method::{StreamSource, encode};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

/// Events queued for one chat stream before its forwarder waits for the connection to catch up.
const CHAT_QUEUE_CAPACITY: usize = 256;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    Ok(vec![
        methods.register(
            Owner::Core,
            chat::START,
            capabilities::AGENT_CHAT,
            Permission::Workspace,
            start,
        )?,
        methods.register(
            Owner::Core,
            chat::APPROVAL_ANSWER,
            capabilities::AGENT_CHAT,
            Permission::Workspace,
            answer,
        )?,
        methods.register(
            Owner::Core,
            chat::INTERRUPT,
            capabilities::AGENT_CHAT,
            Permission::Workspace,
            interrupt,
        )?,
        methods.register_stream(
            Owner::Core,
            chat::SUBSCRIBE,
            capabilities::AGENT_CHAT,
            Permission::Read,
            subscribe,
        )?,
        methods.register(
            Owner::Core,
            chat::HISTORY,
            capabilities::AGENT_CHAT,
            Permission::Read,
            history,
        )?,
        registries
            .capabilities
            .register(capabilities::AGENT_CHAT, Owner::Core)?,
    ])
}

/// Starts a session whose run is a chat (decision 0101, D1). `cols`, `rows` and `args` do not apply to a chat and are
/// refused; a kind that cannot run as one is refused with `agent.chat_unsupported`, whose data is its chat support.
async fn start(ctx: RequestCtx, params: AgentChatStartParams) -> Result<AgentSession, ErrorBody> {
    let start = params.start;
    for (name, given) in [
        ("cols", start.cols.is_some()),
        ("rows", start.rows.is_some()),
        ("args", !start.args.is_empty()),
    ] {
        if given {
            return Err(ErrorBody::new(
                codes::REQUEST_INVALID_PARAMS,
                format!("{name} does not apply to a chat session"),
            ));
        }
    }
    let grant = secret_grant(&ctx);
    let session = on_agents(&ctx, move |agents| agents.start_chat_session(start, grant)).await?;
    Ok(for_caller(&ctx, session))
}

/// Answers a chat session's permission request (decision 0101, D6). An answer reaches the run, so a run that holds vault
/// secrets needs host-admin scope, as a prompt does.
async fn answer(ctx: RequestCtx, params: ChatApprovalAnswerParams) -> Result<ChatItem, ErrorBody> {
    require_admin_for_session(&ctx, params.session.clone()).await?;
    on_agents(&ctx, move |agents| agents.chat_answer(&params)).await
}

/// Interrupts a chat session's running turn and keeps the session (decision 0101, D8).
async fn interrupt(ctx: RequestCtx, params: AgentSessionRef) -> Result<ChatInterrupted, ErrorBody> {
    require_admin_for_session(&ctx, params.session.clone()).await?;
    on_agents(&ctx, move |agents| agents.chat_interrupt(&params.session)).await
}

/// The opening events of a stream (or after a reset), read on the blocking pool.
async fn open(
    agents: &Agents,
    session: String,
    after: Option<ChatCursor>,
    limit: Option<u32>,
) -> Result<ChatOpened, AgentError> {
    let agents = agents.clone();
    match tokio::task::spawn_blocking(move || agents.chat_open(&session, after.as_ref(), limit))
        .await
    {
        Ok(opened) => opened,
        Err(error) => Err(AgentError::new(
            massorca_protocol::codes::INTERNAL,
            format!("the agents task failed: {error}"),
        )),
    }
}

/// Streams a chat session's journal (decision 0101, D4): the opening events, then a `batch` for every change after
/// them, in journal order. The subscription is taken before the opening read, which holds the journal lock, so no change
/// falls between; a batch the opening already covers is skipped. A stream that falls behind gets `reset { overflow }`
/// and a fresh snapshot; a change of epoch gets `reset { epoch_changed }` and one. A forgotten session ends the stream
/// with `end { forgotten }`. A shutdown ends it as it ends every stream: the connection closes each open stream with
/// `service.shutting_down` before anything else (decision 0101, as amended), so no `end { shutting_down }` is sent.
async fn subscribe(
    ctx: RequestCtx,
    params: ChatSubscribeParams,
) -> Result<StreamSource, ErrorBody> {
    // The transcript of a run that holds vault secrets is as guarded as its terminal (decision 0071).
    require_admin_for_session(&ctx, params.session.clone()).await?;
    let mut changes = ctx.state.chat_changes.subscribe();
    let ChatSubscribeParams {
        session,
        after,
        limit,
    } = params;
    let opened = on_agents(&ctx, move |agents| {
        agents.chat_open(&session, after.as_ref(), limit)
    })
    .await?;
    let (items, source) = mpsc::channel(CHAT_QUEUE_CAPACITY);
    let agents = ctx.state.agents.clone();
    tokio::spawn(async move {
        let session = opened.session.clone();
        let mut cursor = opened.cursor.clone();
        let mut pending = opened.events;
        loop {
            for event in pending.drain(..) {
                let Ok(value) = encode(&event) else { return };
                if items.send(value).await.is_err() {
                    return;
                }
            }
            let change = tokio::select! {
                () = items.closed() => return,
                change = changes.recv() => change,
            };
            let reset = match change {
                Ok(Change::Chat(changed)) if changed.session == session => {
                    if changed.batch.cursor.epoch != cursor.epoch {
                        Some(ResetReason::EpochChanged)
                    } else {
                        if changed.batch.cursor.seq > cursor.seq {
                            cursor = changed.batch.cursor.clone();
                            pending.push(ChatEvent::Batch(changed.batch));
                        }
                        None
                    }
                }
                Ok(Change::Forgotten(id)) if id == session => {
                    let end = ChatEvent::End {
                        reason: EndReason::Forgotten,
                    };
                    if let Ok(value) = encode(&end) {
                        let _ = items.send(value).await;
                    }
                    return;
                }
                Ok(_) => None,
                Err(broadcast::error::RecvError::Lagged(_)) => Some(ResetReason::Overflow),
                Err(broadcast::error::RecvError::Closed) => return,
            };
            if let Some(reason) = reset {
                match open(&agents, session.clone(), None, limit).await {
                    Ok(reopened) => {
                        cursor = reopened.cursor;
                        pending.push(ChatEvent::Reset { reason });
                        pending.extend(reopened.events);
                    }
                    Err(_) => return,
                }
            }
        }
    });
    Ok(source)
}

/// Pages a chat session's journal back (decision 0101, D5).
async fn history(ctx: RequestCtx, params: ChatHistoryParams) -> Result<ChatHistoryPage, ErrorBody> {
    require_admin_for_session(&ctx, params.session.clone()).await?;
    on_agents(&ctx, move |agents| agents.chat_history(&params)).await
}
