//! The MassOrca service: one per profile, serving the protocol over a local socket (ADR 0002, 0003), with the registries
//! through which core and extensions contribute methods, capabilities, commands and settings (extensions.md section 9).

mod accept_loop;
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
mod method;
mod owner;
mod permission;
mod registry;
mod service;
mod service_json;
mod session;
mod settings;
mod shutdown;
mod store_blocking;
mod streams;
mod transport;

pub use capability::{CapabilityHandle, CapabilityRegistry};
pub use commands::{CommandDescriptor, CommandRegistry, core_commands};
pub use context::{AppState, Registries, RequestCtx};
pub use error::ServiceError;
pub use event_bus::{EventBus, EventSubscription};
pub use method::{MethodError, MethodInfo, MethodRegistry, StreamSource};
pub use owner::Owner;
pub use permission::{Permission, Principal, authorize};
pub use registry::{Entry, RegistrationError, RegistrationHandle, Registry};
pub use service::{RunningService, Service, ServiceConfig};
pub use settings::{SettingDecl, SettingKind, SettingsRegistry};
pub use shutdown::ShutdownHandle;
