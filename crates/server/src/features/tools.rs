//! `service.tools` (capability `service-tools/1`, decision 0116): the developer tools on this service's host. Detection runs
//! where the service runs, never on the client, and its answer is kept until a request asks for a refresh.

use std::sync::Arc;
use std::time::Duration;

use massorca_core_git::{Git, GitAvailability};
use massorca_core_terminal::shell::{EnvSnapshot, resolve_program};
use massorca_core_tools::catalog::host_tool_specs;
use massorca_core_tools::probe::probe_tools;
use massorca_core_tools::tool_cache::ToolCache;
use massorca_protocol::methods::service::{
    self, ToolInfo, ToolKind, ToolList, ToolState, ToolsParams,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

/// How long each tool gets to print its version. The probes run together, so this bounds the whole detection.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let cache = Arc::new(ToolCache::default());
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::SERVICE_TOOLS, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            service::TOOLS,
            capabilities::SERVICE_TOOLS,
            Permission::Read,
            move |ctx: RequestCtx, params: ToolsParams| {
                let cache = Arc::clone(&cache);
                async move { tools(&cache, &ctx, params).await }
            },
        )?,
    ])
}

/// Detects on the blocking pool: each probe starts a program, which must not stall the reactor.
async fn tools(
    cache: &Arc<ToolCache>,
    ctx: &RequestCtx,
    params: ToolsParams,
) -> Result<ToolList, ErrorBody> {
    let cache = Arc::clone(cache);
    // Why: a fresh handle, so a refresh asks git again instead of reusing the version this service run read first.
    let git = Git::with_binary(ctx.state.git.binary());
    tokio::task::spawn_blocking(move || ToolList {
        tools: cache.report(params.refresh, || detect(&git)),
    })
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "the tool detection did not finish");
        ErrorBody::new(codes::INTERNAL, "the tool detection did not finish")
    })
}

/// Git first, which core-git answers, then the forge CLIs and shells the catalog lists.
fn detect(git: &Git) -> Vec<ToolInfo> {
    let env = EnvSnapshot::current();
    let mut tools = vec![git_tool(git, &env)];
    tools.extend(probe_tools(&host_tool_specs(), &env, PROBE_TIMEOUT));
    tools
}

/// Git's answer from `Git::availability`, which is the same probe `service info` reports.
fn git_tool(git: &Git, env: &EnvSnapshot) -> ToolInfo {
    let (state, version) = match git.availability() {
        GitAvailability::Present(found) => (ToolState::Present, Some(found.to_string())),
        GitAvailability::TooOld(found) => (ToolState::TooOld, Some(found.to_string())),
        GitAvailability::Missing => (ToolState::Missing, None),
        GitAvailability::Unknown(_) => (ToolState::Unknown, None),
    };
    let path = resolve_program(&git.binary().to_string_lossy(), env)
        .map(|path| path.to_string_lossy().into_owned());
    ToolInfo {
        id: "git".to_owned(),
        kind: ToolKind::Vcs,
        state,
        version,
        path,
    }
}
