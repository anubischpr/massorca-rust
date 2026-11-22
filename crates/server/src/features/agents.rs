//! The agent methods (capability `agents/1`, `docs/architecture/agents.md`). The rules live in `core-agents`;
//! handlers run its blocking calls on the blocking pool and publish `agent.changed` after each change.

use massorca_core_agents::{AgentError, Agents};
use massorca_protocol::methods::agents::{
    self, AgentKinds, AgentListParams, AgentSession, AgentSessionList, AgentSessionRef,
    AgentStartParams, AgentStopParams,
};
use massorca_protocol::methods::events::TOPIC_AGENT_CHANGED;
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::NoParams;
use crate::features::workspace_domain::publish;
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
    ])
}

/// Every declared kind, read again from the profile on each call, with programs resolved on the service's `PATH`.
async fn kinds(ctx: RequestCtx, _params: NoParams) -> Result<AgentKinds, ErrorBody> {
    on_agents(&ctx, |agents| Ok(agents.kinds())).await
}

async fn start(ctx: RequestCtx, params: AgentStartParams) -> Result<AgentSession, ErrorBody> {
    let session = on_agents(&ctx, move |agents| agents.start(params)).await?;
    publish(&ctx, TOPIC_AGENT_CHANGED, &session.id);
    Ok(session)
}

async fn list(ctx: RequestCtx, params: AgentListParams) -> Result<AgentSessionList, ErrorBody> {
    let sessions = on_agents(&ctx, move |agents| agents.list(params.workspace.as_deref())).await?;
    Ok(AgentSessionList { sessions })
}

async fn show(ctx: RequestCtx, params: AgentSessionRef) -> Result<AgentSession, ErrorBody> {
    on_agents(&ctx, move |agents| agents.show(&params.session)).await
}

async fn stop(ctx: RequestCtx, params: AgentStopParams) -> Result<AgentSession, ErrorBody> {
    let session = on_agents(&ctx, move |agents| {
        agents.stop(&params.session, params.force)
    })
    .await?;
    publish(&ctx, TOPIC_AGENT_CHANGED, &session.id);
    Ok(session)
}

async fn forget(ctx: RequestCtx, params: AgentSessionRef) -> Result<AgentSession, ErrorBody> {
    let session = on_agents(&ctx, move |agents| agents.forget(&params.session)).await?;
    publish(&ctx, TOPIC_AGENT_CHANGED, &session.id);
    Ok(session)
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
