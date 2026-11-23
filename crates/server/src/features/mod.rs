//! Core features. Each registers its capabilities, methods, commands and settings through the registries, and the service keeps the handles for its lifetime.

mod agents;
mod config;
mod config_keys;
mod devices;
mod events;
pub(crate) mod extension_process;
mod extensions;
mod git;
mod projects;
mod secret_params;
mod secret_refusals;
mod secrets;
mod service;
mod teams;
mod terminal_attach;
mod terminals;
mod workspace_domain;
mod workspaces;

pub(crate) use config::default_workspace_root;
pub(crate) use service::info_of;

use serde::Deserialize;

use crate::context::Registries;
use crate::error::ServiceError;
use crate::permission::{Permission, Principal, authorize};
use crate::registry::RegistrationHandle;

/// Params of a method that takes none. Null and `{}` both decode to it.
#[derive(Debug, Deserialize)]
pub(crate) struct NoParams {}

/// Whether `principal` may see an agent account's personal details: the vendor identity it is signed in as, and the
/// working directory of its sign-in terminal (the account's home). The owner and a device holding `workspace` may; a
/// device below `workspace` gets the same answers with those details left out (decision 0059).
pub(crate) fn sees_account_details(principal: &Principal) -> bool {
    authorize(principal, Permission::Workspace).is_ok()
}

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
    handles.extend(agents::register(registries)?);
    handles.extend(extensions::register(registries)?);
    handles.extend(git::register(registries)?);
    handles.extend(secrets::register(registries)?);
    handles.extend(teams::register(registries)?);
    handles.extend(crate::commands::register_core(&registries.commands)?);
    Ok(handles)
}
