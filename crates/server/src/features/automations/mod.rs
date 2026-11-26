//! Scheduled automation definitions (decision 0123): `automation.create` under capability `automations/1`. A definition
//! is stored here; no run is started.

mod calls;
mod definition;
mod view;

use massorca_protocol::capabilities;
use massorca_protocol::methods::automations;

use crate::context::Registries;
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

/// Registers the automations capability and its methods. Changing a definition is `workspace`, as for routines, since a
/// definition starts agents in a project.
pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    let cap = capabilities::AUTOMATIONS;
    Ok(vec![
        registries.capabilities.register(cap, Owner::Core)?,
        methods.register(
            Owner::Core,
            automations::CREATE,
            cap,
            Permission::Workspace,
            calls::create,
        )?,
    ])
}
