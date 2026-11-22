//! Command descriptors: what each CLI command or palette entry runs (extensions.md section 9, item 5).

mod core_table;
mod git_table;
mod team_table;

use serde_json::Value;

use crate::owner::Owner;
use crate::registry::{RegistrationError, RegistrationHandle, Registry};

/// One command. Its id is the CLI command path, or a palette-only id for a method no CLI command runs; `method` is the request it issues first.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandDescriptor {
    pub id: String,
    pub title: String,
    pub method: String,
    pub args_schema: Value,
}

/// Every core command: the Phase 1 table, then the git and Bot Teams commands.
pub fn core_commands() -> Vec<CommandDescriptor> {
    let mut commands = core_table::core_commands();
    commands.extend(git_table::git_commands());
    commands.extend(team_table::team_commands());
    commands
}

/// Command descriptors keyed by id.
pub type CommandRegistry = Registry<CommandDescriptor>;

/// Registers the core command table under the core owner; the handles stay with the service.
pub(crate) fn register_core(
    registry: &CommandRegistry,
) -> Result<Vec<RegistrationHandle>, RegistrationError> {
    core_commands()
        .into_iter()
        .map(|descriptor| {
            let id = descriptor.id.clone();
            registry.register(Owner::Core, id, descriptor)
        })
        .collect()
}
