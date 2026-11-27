//! The shared state of a running service, and the per-request context that handlers receive.

use std::collections::HashSet;
use std::sync::Arc;

use massorca_core_agents::{Agents, Change};
use massorca_core_extensions::Extensions;
use massorca_core_git::Git;
use massorca_core_store::Store;
use massorca_core_terminal::TerminalManager;
use massorca_core_vault::Vault;
use massorca_core_workspace::Workspaces;
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use tokio::sync::broadcast;

use crate::activity::Activity;
use crate::capability::CapabilityRegistry;
use crate::commands::CommandRegistry;
use crate::event_bus::EventBus;
use crate::method::MethodRegistry;
use crate::network::NetworkState;
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
    /// The host's git for status replies; probed on the first request that needs it, never at start (#169).
    pub git: Git,
    /// The profile's secret vault (decision 0071); calls run on the blocking pool.
    pub vault: Arc<Vault>,
    /// The service's terminals (decision 0033); they end with the service.
    pub terminals: TerminalManager,
    /// Agent sessions and their runs (decision 0051).
    pub agents: Agents,
    /// Installed extensions of the profile (decision 0062). Other features register remove guards and cleanups here.
    pub extensions: Arc<Extensions>,
    /// Finds and runs host programs, such as `glab` for a GitLab connection (decision 0097).
    pub(crate) host_programs: Arc<crate::features::extension_process::HostPrograms>,
    /// Skill discovery's inputs and the scan it shares for ten seconds (SKL-01).
    pub(crate) skills: crate::features::skills::SkillScans,
    /// The binary an update replaces, its supervisor, and the upload in progress (decision 0132).
    pub(crate) self_update: Arc<crate::features::self_update::SelfUpdate>,
    /// Keeps the agents' remove guard registered on `extensions` while the service runs: a package whose kind a live or
    /// resumable session uses is not removed, disabled or updated away.
    pub(crate) _kind_guard: massorca_core_extensions::hooks::HookHandle,
    /// Keeps the agents' remove cleanup registered: a package removed with its agent homes deletes its kinds' accounts.
    pub(crate) _kind_cleanup: massorca_core_extensions::hooks::HookHandle,
    /// Every agent status change with the full record, in the order the store applied them; `agent.watch` reads it.
    pub agent_changes: broadcast::Sender<Change>,
    /// Every chat journal change (`Change::Chat`) and every forgotten session, for the chat streams (decision 0101). Apart
    /// from `agent_changes`, so a fast stream of text never makes a session watcher fall behind.
    pub chat_changes: broadcast::Sender<Change>,
    pub server: PeerInfo,
    /// Listeners, pairing offers and open device connections; empty when the service is local only.
    pub network: NetworkState,
    pub started_at_ms: i64,
    /// Handlers and store calls still running; shutdown waits for them before it releases the profile.
    pub(crate) activity: Activity,
    /// Which workspaces the first-prompt rename has decided (decision 0118); held in memory for the service's run.
    pub(crate) first_prompt_titles: crate::features::first_prompt_rename::TitleBook,
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
