//! The MassOrca service: one per profile, serving the protocol over a local socket (ADR 0002, 0003), with the registries
//! through which core and extensions contribute methods, capabilities, commands and settings (extensions.md section 9).

mod abort_guard;
mod accept_loop;
mod activity;
mod capability;
mod commands;
mod connection;
mod context;
mod dispatch;
mod error;
mod event_bus;
mod features;
mod handshake;
mod instance_lock;
mod kind_packages;
mod method;
mod names;
mod network;
mod owner;
mod permission;
mod registry;
mod service;
mod service_json;
mod session;
mod setting_values;
mod settings;
mod shutdown;
mod start_timing;
mod store_blocking;
mod streams;
mod transport;

pub use capability::{CapabilityHandle, CapabilityRegistry};
pub use commands::{CommandDescriptor, CommandRegistry, core_commands};
pub use context::{AppState, Registries, RequestCtx};
pub use error::ServiceError;
pub use event_bus::{EventBus, EventSubscription};
pub use method::{
    ByteStreamSource, MethodError, MethodInfo, MethodRegistry, StreamItem, StreamSource,
};
pub use network::{ListenSpec, ListenSpecError, NetworkConfig, NetworkState, identity_paths};
pub use owner::Owner;
pub use permission::{Permission, Principal, Scopes, authorize};
pub use registry::{Entry, RegistrationError, RegistrationHandle, Registry};
pub use service::{RunningService, Service, ServiceConfig, VAULT_PROTECTION_ENV, VaultOptions};
pub use settings::{SettingDecl, SettingKind, SettingsRegistry};
pub use shutdown::ShutdownHandle;
pub use start_timing::START_TIMING_ENV;
