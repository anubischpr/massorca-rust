//! `massorca host ...`: the hosts this client paired with (decision 0046), and installing new ones over SSH.

pub mod add_ssh;
pub mod discover;
pub mod memory;
pub mod records;
pub mod status;

use massorca_profile::Profile;

use crate::args::HostCommand;
use crate::error::CliError;
use crate::output::Output;

/// The command path a host subcommand is named by, for the `--host` refusal.
pub fn command_path(command: &HostCommand) -> &'static str {
    match command {
        HostCommand::List => "host list",
        HostCommand::Show { .. } => "host show",
        HostCommand::Remove(_) => "host remove",
        HostCommand::Status { .. } => "host status",
        HostCommand::Discover => "host discover",
        HostCommand::AddSsh(_) => "host add-ssh",
        HostCommand::Memory => "host memory",
    }
}

/// Runs one `host` subcommand against the hosts of `profile`. `host memory` addresses a service instead; the caller
/// runs it through [`memory::execute`].
pub async fn execute(
    profile: &Profile,
    command: HostCommand,
    output: Output,
) -> Result<(), CliError> {
    match command {
        HostCommand::List => records::list(profile, output),
        HostCommand::Show { name } => records::show(profile, &name, output),
        HostCommand::Remove(args) => records::remove(profile, &args, output).await,
        HostCommand::Status { name } => status::execute(profile, name.as_deref(), output).await,
        HostCommand::Discover => discover::execute(profile, output).await,
        HostCommand::AddSsh(args) => add_ssh::execute(profile, &args, output).await,
        HostCommand::Memory => Err(CliError::new(
            "internal",
            "host memory addresses a service; it does not run on the hosts list",
            crate::error::exit::ERROR,
        )),
    }
}
