//! Scheduled automation definitions (decision 0123): `automation.create`, `list`, `show`, `update`, `rm`, `enable` and
//! `disable` under capability `automations/1`. A definition is stored and listed here; no run is started.

mod calls;
mod definition;
mod occurrences;
mod selectors;
mod view;

pub(crate) use occurrences::upcoming_runs;

use massorca_protocol::capabilities;
use massorca_protocol::methods::automations;

use crate::context::Registries;
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

/// Registers the automations capability and its methods. Reading is `read`; changing a definition is `workspace`,
/// as for routines, since a definition starts agents in a project.
pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    let cap = capabilities::AUTOMATIONS;
    Ok(vec![
        registries.capabilities.register(cap, Owner::Core)?,
        methods.register(
            Owner::Core,
            automations::LIST,
            cap,
            Permission::Read,
            calls::list,
        )?,
        methods.register(
            Owner::Core,
            automations::SHOW,
            cap,
            Permission::Read,
            calls::show,
        )?,
        methods.register(
            Owner::Core,
            automations::CREATE,
            cap,
            Permission::Workspace,
            calls::create,
        )?,
        methods.register(
            Owner::Core,
            automations::UPDATE,
            cap,
            Permission::Workspace,
            calls::update,
        )?,
        methods.register(
            Owner::Core,
            automations::REMOVE,
            cap,
            Permission::Workspace,
            calls::remove,
        )?,
        methods.register(
            Owner::Core,
            automations::ENABLE,
            cap,
            Permission::Workspace,
            calls::enable,
        )?,
        methods.register(
            Owner::Core,
            automations::DISABLE,
            cap,
            Permission::Workspace,
            calls::disable,
        )?,
    ])
}
