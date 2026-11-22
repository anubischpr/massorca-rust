//! The shared state of a running service, and the per-request context that handlers receive.

use std::collections::HashSet;
use std::sync::Arc;

use massorca_core_agents::{Agents, Change};
use massorca_core_store::Store;
use massorca_core_terminal::TerminalManager;
use massorca_core_workspace::Workspaces;
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use tokio::sync::broadcast;

use crate::activity::Activity;
use crate::capability::CapabilityRegistry;
use crate::commands::CommandRegistry;
use crate::event_bus::EventBus;
use crate::method::MethodRegistry;
use crate::permission::Principal;
use crate::settings::SettingsRegistry;
use crate::shutdown::ShutdownHandle;

/// Everything the service offers. Core registers into these at start; extensions register into them the same way.
#[derive(Clone, Debug, Default)]
pub struct Registries {
    pub capabilities: CapabilityRegistry,
    pub methods: MethodRegistry,
    pub commands: CommandRegistry,
    pub settings: SettingsRegistry,
}

/// The state shared by one running service and all its connections.
#[derive(Debug)]
pub struct AppState {
    pub profile: Profile,
    pub store: Store,
    pub events: EventBus,
    pub shutdown: ShutdownHandle,
    pub registries: Registries,
    /// The projects and workspaces domain. Calls take a handle with the current default root.
    pub workspaces: Arc<Workspaces>,
    /// The service's terminals (decision 0033); they end with the service.
    pub terminals: TerminalManager,
    /// Agent sessions and their runs (decision 0051).
    pub agents: Agents,
    /// Every agent status change with the full record, in the order the store applied them; `agent.watch` reads it.
    pub agent_changes: broadcast::Sender<Change>,
    pub server: PeerInfo,
    pub started_at_ms: i64,
    /// Handlers and store calls still running; shutdown waits for them before it releases the profile.
    pub(crate) activity: Activity,
}

/// The context a handler receives. The principal is read from here only.
#[derive(Debug, Clone)]
pub struct RequestCtx {
    pub principal: Principal,
    pub state: Arc<AppState>,
    pub connection_id: String,
    /// The capabilities the client listed in its `hello`. The service does not act on them yet.
    pub client_capabilities: Arc<HashSet<String>>,
}
