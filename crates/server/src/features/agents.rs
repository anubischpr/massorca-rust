//! The agent methods (capability `agents/1`, `docs/architecture/agents.md`). The rules live in `core-agents`;
//! handlers run its blocking calls on the blocking pool. `core-agents` announces every status change; the service
//! turns each into `agent.changed` on the event bus and an item for `agent.watch` (0052).

use std::path::{Path, PathBuf};

use massorca_core_agents::{AgentError, Agents, Change, credentials};
use massorca_protocol::methods::agents::{
    self, AgentHookParams, AgentKinds, AgentListParams, AgentSession, AgentSessionList,
    AgentSessionRef, AgentStartParams, AgentStopParams, AgentWatchEvent, AgentWatchParams,
    CredentialsImportParams, CredentialsImported, CredentialsKindRef, CredentialsRemoved,
    CredentialsShown,
};
use massorca_protocol::{ErrorBody, capabilities, codes};
use tokio::sync::{broadcast, mpsc};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::NoParams;
use crate::method::{StreamSource, encode};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    let agents_cap = capabilities::AGENTS;
    Ok(vec![
        registries.capabilities.register(agents_cap, Owner::Core)?,
        methods.register(
            Owner::Core,
            agents::KINDS,
            agents_cap,
            Permission::Read,
            kinds,
        )?,
        methods.register(
            Owner::Core,
            agents::START,
            agents_cap,
            Permission::Workspace,
            start,
        )?,
        methods.register(
            Owner::Core,
            agents::LIST,
            agents_cap,
            Permission::Read,
            list,
        )?,
        methods.register(
            Owner::Core,
            agents::SHOW,
            agents_cap,
            Permission::Read,
            show,
        )?,
        methods.register(
            Owner::Core,
            agents::STOP,
            agents_cap,
            Permission::Workspace,
            stop,
        )?,
        methods.register(
            Owner::Core,
            agents::FORGET,
            agents_cap,
            Permission::Workspace,
            forget,
        )?,
        methods.register(
            Owner::Core,
            agents::HOOK,
            agents_cap,
            Permission::Workspace,
            hook,
        )?,
        methods.register_stream(
            Owner::Core,
            agents::WATCH,
            agents_cap,
            Permission::Read,
            watch,
        )?,
        methods.register(
            Owner::Core,
            agents::CREDENTIALS_IMPORT,
            agents_cap,
            Permission::Workspace,
            credentials_import,
        )?,
        methods.register(
            Owner::Core,
            agents::CREDENTIALS_SHOW,
            agents_cap,
            Permission::Read,
            credentials_show,
        )?,
        methods.register(
            Owner::Core,
            agents::CREDENTIALS_REMOVE,
            agents_cap,
            Permission::Workspace,
            credentials_remove,
        )?,
    ])
}

/// Items queued for one watcher before its forwarder waits for the connection to catch up.
const WATCH_QUEUE_CAPACITY: usize = 64;

/// Every declared kind, read again from the profile on each call, with programs resolved on the service's `PATH`.
async fn kinds(ctx: RequestCtx, _params: NoParams) -> Result<AgentKinds, ErrorBody> {
    on_agents(&ctx, |agents| Ok(agents.kinds())).await
}

async fn start(ctx: RequestCtx, params: AgentStartParams) -> Result<AgentSession, ErrorBody> {
    on_agents(&ctx, move |agents| agents.start(params)).await
}

async fn list(ctx: RequestCtx, params: AgentListParams) -> Result<AgentSessionList, ErrorBody> {
    let sessions = on_agents(&ctx, move |agents| agents.list(params.workspace.as_deref())).await?;
    Ok(AgentSessionList { sessions })
}

async fn show(ctx: RequestCtx, params: AgentSessionRef) -> Result<AgentSession, ErrorBody> {
    on_agents(&ctx, move |agents| agents.show(&params.session)).await
}

async fn stop(ctx: RequestCtx, params: AgentStopParams) -> Result<AgentSession, ErrorBody> {
    on_agents(&ctx, move |agents| {
        agents.stop(&params.session, params.force)
    })
    .await
}

async fn forget(ctx: RequestCtx, params: AgentSessionRef) -> Result<AgentSession, ErrorBody> {
    on_agents(&ctx, move |agents| agents.forget(&params.session)).await
}

/// A report from a run's hook (0053). The payload, the result and the token are never logged.
async fn hook(ctx: RequestCtx, params: AgentHookParams) -> Result<(), ErrorBody> {
    on_agents(&ctx, move |agents| agents.hook(&params)).await
}

/// The sessions a watcher selected, as ids resolved once when the stream opens.
#[derive(Debug, Clone)]
struct Selection {
    workspace_id: Option<String>,
    session_id: Option<String>,
}

impl Selection {
    fn resolve(agents: &Agents, params: &AgentWatchParams) -> Result<Self, AgentError> {
        let workspace_id = match &params.workspace {
            Some(selector) => Some(agents.workspace_id(selector)?),
            None => None,
        };
        let session_id = match &params.session {
            Some(selector) => Some(agents.show(selector)?.id),
            None => None,
        };
        Ok(Self {
            workspace_id,
            session_id,
        })
    }

    fn selects(&self, session: &AgentSession) -> bool {
        self.workspace_id
            .as_ref()
            .is_none_or(|id| *id == session.workspace_id)
            && self.session_id.as_ref().is_none_or(|id| *id == session.id)
    }

    /// Every selected session now, newest first.
    fn snapshot(&self, agents: &Agents) -> Result<AgentWatchEvent, AgentError> {
        let sessions = agents
            .list(self.workspace_id.as_deref())?
            .into_iter()
            .filter(|session| self.selects(session))
            .collect();
        Ok(AgentWatchEvent::Snapshot { sessions })
    }
}

/// Streams the selected sessions: a `snapshot` first, then a `changed` item after every change, in the order the store
/// applied them. The subscription is taken before the snapshot, so no change falls between; a watcher that falls
/// behind is given a fresh `snapshot` (0052).
async fn watch(ctx: RequestCtx, params: AgentWatchParams) -> Result<StreamSource, ErrorBody> {
    let mut changes = ctx.state.agent_changes.subscribe();
    let (selection, first) = on_agents(&ctx, move |agents| {
        let selection = Selection::resolve(agents, &params)?;
        let first = selection.snapshot(agents)?;
        Ok((selection, first))
    })
    .await?;
    let (items, source) = mpsc::channel(WATCH_QUEUE_CAPACITY);
    let agents = ctx.state.agents.clone();
    tokio::spawn(async move {
        let mut next = Some(first);
        loop {
            if let Some(item) = next.take() {
                let Ok(value) = encode(&item) else { break };
                if items.send(value).await.is_err() {
                    break;
                }
            }
            let change = tokio::select! {
                () = items.closed() => break,
                change = changes.recv() => change,
            };
            next = match change {
                Ok(Change::Updated(session)) if selection.selects(&session) => {
                    Some(AgentWatchEvent::Changed { session })
                }
                Ok(_) => None,
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    let agents = agents.clone();
                    let selection = selection.clone();
                    match tokio::task::spawn_blocking(move || selection.snapshot(&agents)).await {
                        Ok(Ok(snapshot)) => Some(snapshot),
                        _ => break,
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
            };
        }
    });
    Ok(source)
}

/// Copies a kind's declared credential files into the profile's home for it (0056). Only names and sizes come back;
/// nothing about the files is logged beyond that.
async fn credentials_import(
    ctx: RequestCtx,
    params: CredentialsImportParams,
) -> Result<CredentialsImported, ErrorBody> {
    on_profile(&ctx, move |profile_dir| {
        credentials::import(
            profile_dir,
            &params.kind,
            Path::new(&params.from),
            params.replace,
        )
    })
    .await
}

async fn credentials_show(
    ctx: RequestCtx,
    params: CredentialsKindRef,
) -> Result<CredentialsShown, ErrorBody> {
    on_profile(&ctx, move |profile_dir| {
        credentials::show(profile_dir, &params.kind)
    })
    .await
}

async fn credentials_remove(
    ctx: RequestCtx,
    params: CredentialsKindRef,
) -> Result<CredentialsRemoved, ErrorBody> {
    on_profile(&ctx, move |profile_dir| {
        credentials::remove(profile_dir, &params.kind)
    })
    .await
}

/// Runs one call on the profile directory on the blocking pool and maps its refusal to the protocol error.
async fn on_profile<T, F>(ctx: &RequestCtx, call: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce(&Path) -> Result<T, AgentError> + Send + 'static,
{
    let profile_dir: PathBuf = ctx.state.profile.dir().to_path_buf();
    match tokio::task::spawn_blocking(move || call(&profile_dir)).await {
        Ok(result) => result.map_err(ErrorBody::from),
        Err(error) => Err(ErrorBody::new(
            codes::INTERNAL,
            format!("the credentials task failed: {error}"),
        )),
    }
}

/// Runs one agents call on the blocking pool and maps its refusal to the protocol error.
async fn on_agents<T, F>(ctx: &RequestCtx, call: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce(&Agents) -> Result<T, AgentError> + Send + 'static,
{
    let agents = ctx.state.agents.clone();
    match tokio::task::spawn_blocking(move || call(&agents)).await {
        Ok(result) => result.map_err(ErrorBody::from),
        Err(error) => Err(ErrorBody::new(
            codes::INTERNAL,
            format!("the agents task failed: {error}"),
        )),
    }
}
