//! `massorca host ...`: the hosts this client paired with (decision 0046), and installing new ones over SSH.

pub mod add_ssh;
pub mod discover;
pub mod memory;
pub mod ports;
pub mod records;
pub mod self_test;
pub mod status;
pub mod status_watch;

use massorca_profile::Profile;

use crate::args::HostCommand;
use crate::error::CliError;
use crate::output::Output;

/// The failures that mean the service could not be reached or was lost mid-call. A host figure is then unknown, never
/// zero: `memory.unverifiable` and `ports.unverifiable` keep the original code in their data.
pub(crate) const LOST_CONTACT: [&str; 4] = [
    "service.unavailable",
    "service.disconnected",
    "host.unreachable",
    "client.io",
];

/// The command path a host subcommand is named by, for the `--host` refusal.
pub fn command_path(command: &HostCommand) -> &'static str {
    match command {
        HostCommand::List => "host list",
        HostCommand::Show { .. } => "host show",
        HostCommand::Remove(_) => "host remove",
        HostCommand::Status { .. } => "host status",
        HostCommand::SelfTest { .. } => "host self-test",
        HostCommand::Discover => "host discover",
        HostCommand::AddSsh(_) => "host add-ssh",
        HostCommand::Memory => "host memory",
        HostCommand::Ports => "host ports",
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
        HostCommand::Status { name, watch: false } => {
            status::execute(profile, name.as_deref(), output).await
        }
        HostCommand::Status { name, watch: true } => {
            status_watch::execute(profile, name.as_deref(), output).await
        }
        HostCommand::SelfTest { name } => {
            self_test::execute(profile, name.as_deref(), output).await
        }
        HostCommand::Discover => discover::execute(profile, output).await,
        HostCommand::AddSsh(args) => add_ssh::execute(profile, &args, output).await,
        HostCommand::Memory => Err(CliError::new(
            "internal",
            "host memory addresses a service; it does not run on the hosts list",
            crate::error::exit::ERROR,
        )),
        HostCommand::Ports => Err(CliError::new(
            "internal",
            "host ports addresses a service; it does not run on the hosts list",
            crate::error::exit::ERROR,
        )),
    }
}
