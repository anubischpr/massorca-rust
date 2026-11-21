//! `service.info` and `service.shutdown` (capability `service/1`).

use std::time::Duration;

use massorca_protocol::methods::service::{self, ServiceInfo};
use massorca_protocol::{ErrorBody, PROTOCOL_VERSION, capabilities};
use tokio::time::timeout;

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
        // Offered to every client: the frame kind is the transport's, so the service offers it as its own capability.
        registries
            .capabilities
            .register(capabilities::BINARY_FRAMES, Owner::Core)?,
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

/// Requests shutdown and answers once the service has stopped listening, so a client that connects right after the answer
/// cannot reach the old service. The service then winds down.
async fn shutdown(ctx: RequestCtx, _params: NoParams) -> Result<(), ErrorBody> {
    ctx.state.shutdown.shutdown();
    // Bounded, so a listener that never closes cannot leave this request unanswered.
    let _ = timeout(Duration::from_secs(5), ctx.state.shutdown.listener_closed()).await;
    Ok(())
}
