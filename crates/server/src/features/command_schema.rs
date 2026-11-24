//! `commands.describe`: the command table, each command with the capability and permission of the method it issues.

use massorca_protocol::ErrorBody;
use massorca_protocol::capabilities::COMMANDS;
use massorca_protocol::methods::commands::{self, CommandEntry, CommandTable};

use super::NoParams;
use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries.capabilities.register(COMMANDS, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            commands::DESCRIBE,
            COMMANDS,
            Permission::Read,
            describe,
        )?,
    ])
}

/// Every core command in id order. The method's capability and permission come from the method registry, not the table.
async fn describe(ctx: RequestCtx, _params: NoParams) -> Result<CommandTable, ErrorBody> {
    let registries = &ctx.state.registries;
    let commands: Vec<CommandEntry> = registries
        .commands
        .list()
        .iter()
        .map(|entry| {
            let descriptor = &entry.value;
            let method = registries.methods.lookup(&descriptor.method);
            CommandEntry {
                id: descriptor.id.clone(),
                title: descriptor.title.clone(),
                method: descriptor.method.clone(),
                capability: method.as_ref().map(|found| found.value.capability.clone()),
                permission: method
                    .as_ref()
                    .map(|found| found.value.permission.as_str().to_owned()),
                args_schema: descriptor.args_schema.clone(),
            }
        })
        .collect();
    Ok(CommandTable { commands })
}
