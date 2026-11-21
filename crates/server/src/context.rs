//! The shared state of a running service, and the per-request context that handlers receive.

use std::sync::Arc;

use massorca_core_store::Store;
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;

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
    pub server: PeerInfo,
    pub started_at_ms: i64,
}

/// The context a handler receives. The principal is read from here only.
#[derive(Debug, Clone)]
pub struct RequestCtx {
    pub principal: Principal,
    pub state: Arc<AppState>,
    pub connection_id: String,
}
