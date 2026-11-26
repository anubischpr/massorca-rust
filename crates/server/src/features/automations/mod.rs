//! Scheduled automations: their definitions (decision 0123), `automation.create`, `list`, `show`, `update`, `rm`,
//! `enable` and `disable` under capability `automations/1`; and their runs (decision 0125): the agent launch,
//! `automation.run_now` and `automation.runs` under capability `automation-runs/1`.

mod calls;
mod definition;
mod launch;
mod occurrences;
mod run_calls;
mod runs;
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
        registries
            .capabilities
            .register(capabilities::AUTOMATION_RUNS, Owner::Core)?,
        methods.register(
            Owner::Core,
            automations::RUN_NOW,
            capabilities::AUTOMATION_RUNS,
            Permission::Workspace,
            run_calls::run_now,
        )?,
        methods.register(
            Owner::Core,
            automations::RUNS,
            capabilities::AUTOMATION_RUNS,
            Permission::Read,
            run_calls::runs,
        )?,
    ])
}
