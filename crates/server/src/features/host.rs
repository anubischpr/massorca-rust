//! `host.memory` (capability `host/1`, decision 0081): the memory of the service and of each live terminal's processes,
//! read on this host when asked. Nothing is pushed; clients poll.

use std::collections::HashMap;

use massorca_core_agents::sessions::Agents;
use massorca_core_store::now_ms;
use massorca_core_terminal::TerminalManager;
use massorca_host_stats::{self as stats, Metric};
use massorca_protocol::methods::host::{
    self, HostMemory, MemoryMetric, ProcessMemory, TerminalMemory,
};
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
            .register(capabilities::HOST, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            host::MEMORY,
            capabilities::HOST,
            Permission::Read,
            memory,
        )?,
    ])
}

/// Samples on the blocking pool: listing processes and reading each one are system calls, never a spawned program.
async fn memory(ctx: RequestCtx, _params: NoParams) -> Result<HostMemory, ErrorBody> {
    let terminals = ctx.state.terminals.clone();
    let agents = ctx.state.agents.clone();
    match tokio::task::spawn_blocking(move || sample(&terminals, &agents)).await {
        Ok(answer) => answer,
        Err(error) => Err(ErrorBody::new(
            codes::INTERNAL,
            format!("the memory sample failed: {error}"),
        )),
    }
}

fn sample(terminals: &TerminalManager, agents: &Agents) -> Result<HostMemory, ErrorBody> {
    let own_pid = std::process::id();
    // The service always reads its own process; failing to is a platform without a reader, not an empty figure.
    let own_bytes = stats::memory_of(own_pid).ok_or_else(|| {
        ErrorBody::new(codes::INTERNAL, "this host's process memory cannot be read")
    })?;
    let sessions = sessions_by_terminal(agents);
    let mut unreadable = 0u32;
    let mut total_bytes = own_bytes;
    let entries = terminals
        .processes()
        .into_iter()
        .map(|terminal| {
            let sample = stats::sample_memory(&terminal.pids);
            unreadable = unreadable.saturating_add(sample.unreadable);
            let bytes = sample.total_bytes();
            total_bytes = total_bytes.saturating_add(bytes);
            TerminalMemory {
                session_id: sessions.get(&terminal.terminal_id).cloned(),
                terminal_id: terminal.terminal_id,
                workspace_id: terminal.workspace_id,
                processes: u32::try_from(sample.processes.len()).unwrap_or(u32::MAX),
                bytes,
            }
        })
        .collect();
    Ok(HostMemory {
        metric: metric_of(stats::METRIC),
        total_bytes,
        service: ProcessMemory {
            pid: own_pid,
            bytes: own_bytes,
        },
        terminals: entries,
        unreadable,
        sampled_at_ms: now_ms(),
    })
}

/// The agent session whose current run uses each terminal. A session's row names only its current run's terminal,
/// and only live terminals are sampled, so a match is that session's live run. The labels are optional: when the
/// sessions cannot be read the figures are still answered, without them.
fn sessions_by_terminal(agents: &Agents) -> HashMap<String, String> {
    match agents.list(None) {
        Ok(sessions) => sessions
            .into_iter()
            .filter_map(|session| Some((session.terminal_id?, session.id)))
            .collect(),
        Err(error) => {
            tracing::warn!(
                code = error.code,
                "host memory could not read the agent sessions"
            );
            HashMap::new()
        }
    }
}

fn metric_of(metric: Metric) -> MemoryMetric {
    match metric {
        Metric::WorkingSet => MemoryMetric::WorkingSet,
        Metric::Resident => MemoryMetric::Resident,
    }
}
