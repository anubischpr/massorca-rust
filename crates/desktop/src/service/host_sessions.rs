//! The agent sessions a paired host runs (STAT-16). Each check of a host reads them on the connection that
//! checks it. A host that cannot be reached keeps the sessions it last ran, unverified: losing contact is never
//! proof that a run ended.

use std::sync::atomic::{AtomicU64, Ordering};

use massorca_client::Client;
use massorca_protocol::methods::agents::AgentSession;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceListParams};

use super::agents::list_sessions;

static CHECKS: AtomicU64 = AtomicU64::new(0);

/// Numbers one check of the paired hosts. A later check always has a larger number.
pub(super) fn next_check() -> u64 {
    CHECKS.fetch_add(1, Ordering::Relaxed) + 1
}

/// What a host answered for one check: its sessions, and the workspaces they run in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostAgents {
    pub sessions: Vec<AgentSession>,
    pub workspaces: Vec<Workspace>,
}

/// One check's answer about a host's sessions. `answer` is `None` when the host could not be read on this check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostSessionsRead {
    pub host: String,
    pub request: u64,
    pub answer: Option<HostAgents>,
}

/// The sessions a paired host last reported. `reached` says whether the newest check reached it; while it is
/// false the sessions are last known and unverifiable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostSessions {
    pub request: u64,
    pub reached: bool,
    pub sessions: Vec<AgentSession>,
    pub workspaces: Vec<Workspace>,
}

/// Reads a host's sessions and workspaces on its own open connection. `None` when the sessions could not be
/// listed, so the host counts as not reached.
pub(super) async fn read(client: &Client) -> Option<HostAgents> {
    let (sessions, workspaces) = tokio::join!(
        list_sessions(client),
        client.workspace_list(&WorkspaceListParams { project: None })
    );
    let sessions = sessions.ok()?;
    let workspaces = workspaces
        .map(|listed| listed.workspaces)
        .unwrap_or_default();
    Some(HostAgents {
        sessions,
        workspaces,
    })
}

#[cfg(test)]
mod tests {
    use super::next_check;

    #[test]
    fn a_later_check_has_a_larger_number() {
        let first = next_check();
        let second = next_check();
        assert!(second > first);
    }
}
