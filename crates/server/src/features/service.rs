//! `service.info` and `service.shutdown` (capability `service/1`).

use massorca_protocol::methods::service::{self, ServiceInfo};
use massorca_protocol::{ErrorBody, PROTOCOL_VERSION, capabilities};

use crate::context::{AppState, Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::NoParams;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::SERVICE, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            service::INFO,
            capabilities::SERVICE,
            Permission::Read,
            info,
        )?,
        registries.methods.register(
            Owner::Core,
            service::SHUTDOWN,
            capabilities::SERVICE,
            Permission::HostAdmin,
            shutdown,
        )?,
    ])
}

/// What `service.info` reports. `RunningService::info` reads the same source.
pub(crate) fn info_of(state: &AppState) -> ServiceInfo {
    ServiceInfo {
        version: state.server.version.clone(),
        protocol: PROTOCOL_VERSION,
        capabilities: state.registries.capabilities.names(),
        profile: state.profile.name().to_owned(),
        pid: std::process::id(),
        started_at_ms: state.started_at_ms,
    }
}

async fn info(ctx: RequestCtx, _params: NoParams) -> Result<ServiceInfo, ErrorBody> {
    Ok(info_of(&ctx.state))
}

/// Requests shutdown and returns; the connection answers first, then the service winds down.
async fn shutdown(ctx: RequestCtx, _params: NoParams) -> Result<(), ErrorBody> {
    ctx.state.shutdown.shutdown();
    Ok(())
}
