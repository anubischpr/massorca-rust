//! The MassOrca service: one per profile, serving the protocol over a local socket (ADR 0002, 0003), with the registries
//! through which core and extensions contribute methods, capabilities, commands and settings (extensions.md section 9).

mod capability;
mod commands;
mod context;
mod event_bus;
mod method;
mod owner;
mod permission;
mod registry;
mod settings;
mod shutdown;

pub use capability::{CapabilityHandle, CapabilityRegistry};
pub use commands::{CommandDescriptor, CommandRegistry, core_commands};
pub use context::{AppState, Registries, RequestCtx};
pub use event_bus::{EventBus, EventSubscription};
pub use method::{MethodError, MethodInfo, MethodRegistry, StreamSource};
pub use owner::Owner;
pub use permission::{Permission, Principal, authorize};
pub use registry::{Entry, RegistrationError, RegistrationHandle, Registry};
pub use settings::{SettingDecl, SettingKind, SettingsRegistry};
pub use shutdown::ShutdownHandle;
