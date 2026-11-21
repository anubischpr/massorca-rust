//! The MassOrca service: one per profile, serving the protocol over a local socket (ADR 0002, 0003), with the registries
//! through which core and extensions contribute methods, capabilities, commands and settings (extensions.md section 9).

mod capability;
mod commands;
mod owner;
mod registry;
mod settings;

pub use capability::{CapabilityHandle, CapabilityRegistry};
pub use commands::{CommandDescriptor, CommandRegistry, core_commands};
pub use owner::Owner;
pub use registry::{Entry, RegistrationError, RegistrationHandle, Registry};
pub use settings::{SettingDecl, SettingKind, SettingsRegistry};
