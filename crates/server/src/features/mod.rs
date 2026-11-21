//! Core features. Each registers its capabilities, methods, commands and settings through the registries, and the service keeps the handles for its lifetime.

mod config;
mod devices;
mod events;
mod projects;
mod service;
mod terminal_attach;
mod terminals;
mod workspace_domain;
mod workspaces;

pub(crate) use config::default_workspace_root;
pub(crate) use service::info_of;

use serde::Deserialize;

use crate::context::Registries;
use crate::error::ServiceError;
use crate::registry::RegistrationHandle;

/// Params of a method that takes none. Null and `{}` both decode to it.
#[derive(Debug, Deserialize)]
pub(crate) struct NoParams {}

/// Registers every core feature and returns the handles that keep them offered.
pub(crate) fn register_core(
    registries: &Registries,
) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let mut handles = service::register(registries)?;
    handles.extend(events::register(registries)?);
    handles.extend(config::register(registries)?);
    handles.extend(projects::register(registries)?);
    handles.extend(devices::register(registries)?);
    handles.extend(workspaces::register(registries)?);
    handles.extend(terminals::register(registries)?);
    handles.extend(terminal_attach::register(registries)?);
    handles.extend(crate::commands::register_core(&registries.commands)?);
    Ok(handles)
}
