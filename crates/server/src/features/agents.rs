//! The agent methods (capability `agents/1`, `docs/architecture/agents.md`). The rules live in `core-agents`;
//! handlers run its blocking calls on the blocking pool.

use massorca_core_agents::{Catalog, SearchEnv};
use massorca_protocol::methods::agents::{self, AgentKinds};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::NoParams;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::AGENTS, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            agents::KINDS,
            capabilities::AGENTS,
            Permission::Read,
            kinds,
        )?,
    ])
}

/// Every declared kind, read again from the profile on each call, with programs resolved on the service's `PATH`.
async fn kinds(ctx: RequestCtx, _params: NoParams) -> Result<AgentKinds, ErrorBody> {
    let profile_dir = ctx.state.profile.dir().to_path_buf();
    tokio::task::spawn_blocking(move || AgentKinds {
        kinds: Catalog::load(&profile_dir).views(&profile_dir, &SearchEnv::current()),
    })
    .await
    .map_err(|error| ErrorBody::new(codes::INTERNAL, format!("the agents task failed: {error}")))
}
