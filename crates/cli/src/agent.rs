//! `massorca agent ...`: coding agents run by the service in workspaces (`docs/architecture/agents.md`).

use clap::Subcommand;
use massorca_profile::Profile;

use crate::agent_text;
use crate::connect::connect;
use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Subcommand)]
pub enum AgentCommand {
    /// Lists the agent kinds: built in and declared in the profile, whether each is installed, and any problems.
    Kinds,
}

/// Runs one `agent` subcommand against the service of `profile`.
pub async fn execute(
    profile: &Profile,
    autostart: bool,
    command: AgentCommand,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(profile, autostart).await?;
    match command {
        AgentCommand::Kinds => {
            let kinds = client.agent_kinds().await?;
            output.success(&kinds, &agent_text::kinds_lines(&kinds.kinds));
        }
    }
    Ok(())
}
