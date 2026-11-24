//! `service.info` and `service.shutdown` (capability `service/1`).

use std::time::Duration;

use massorca_core_git::{GitAvailability, GitVersion};
use massorca_protocol::methods::service::{self, GitStatus, ServiceInfo};
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

/// What `service.info` reports. `RunningService::info` reads the same source. It never probes git: the request probes it.
pub(crate) fn info_of(state: &AppState) -> ServiceInfo {
    ServiceInfo {
        version: state.server.version.clone(),
        protocol: PROTOCOL_VERSION,
        capabilities: state.registries.capabilities.names(),
        profile: state.profile.name().to_owned(),
        pid: std::process::id(),
        started_at_ms: state.started_at_ms,
        git: None,
    }
}

/// Probes the host's git on the blocking pool, so a slow git cannot stall the reactor (ADR 0014).
async fn info(ctx: RequestCtx, _params: NoParams) -> Result<ServiceInfo, ErrorBody> {
    let mut info = info_of(&ctx.state);
    let git = ctx.state.git.clone();
    let status = tokio::task::spawn_blocking(move || git_status(git.availability()))
        .await
        .unwrap_or_else(|error| GitStatus::Unknown {
            reason: error.to_string(),
        });
    info.git = Some(status);
    Ok(info)
}

fn git_status(availability: GitAvailability) -> GitStatus {
    match availability {
        GitAvailability::Present(found) => GitStatus::Present {
            version: found.to_string(),
        },
        GitAvailability::Missing => GitStatus::Missing,
        GitAvailability::TooOld(found) => GitStatus::TooOld {
            version: found.to_string(),
            required: GitVersion::BASELINE.to_string(),
        },
        GitAvailability::Unknown(reason) => GitStatus::Unknown { reason },
    }
}

/// Requests shutdown and answers once the service has stopped listening, so a client that connects right after the answer
/// cannot reach the old service. The service then winds down.
async fn shutdown(ctx: RequestCtx, _params: NoParams) -> Result<(), ErrorBody> {
    ctx.state.shutdown.shutdown();
    // Bounded, so a listener that never closes cannot leave this request unanswered.
    let _ = timeout(Duration::from_secs(5), ctx.state.shutdown.listener_closed()).await;
    Ok(())
}
