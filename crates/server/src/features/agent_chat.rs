//! The chat methods (decision 0101): `agent.chat.subscribe` streams a chat session's journal, `agent.chat.history`
//! pages it back. The rules and the journal live in `core-agents`; its journal changes reach the streams through the
//! service's chat channel, which carries `Change::Chat` and `Change::Forgotten`.
//!
//! The methods are registered under `agent-chat/1`, which the service does not offer yet: it offers the capability once
//! it serves every chat method, `agent.chat.start` included (D11). A test service that starts fake kinds as chat runs
//! offers it, so its tests can reach these methods.

use massorca_core_agents::chat::test_chat_enabled;
use massorca_core_agents::{AgentError, Agents, Change, ChatOpened};
use massorca_protocol::methods::chat::{
    self, ChatCursor, ChatEvent, ChatHistoryPage, ChatHistoryParams, ChatSubscribeParams,
    EndReason, ResetReason,
};
use massorca_protocol::{ErrorBody, capabilities};
use tokio::sync::{broadcast, mpsc};

use super::agents::{on_agents, require_admin_for_session};
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
    let mut handles = vec![
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
    ];
    if test_chat_enabled() {
        handles.push(
            registries
                .capabilities
                .register(capabilities::AGENT_CHAT, Owner::Core)?,
        );
    }
    Ok(handles)
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
