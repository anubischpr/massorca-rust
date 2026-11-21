//! `massorca git ...`: the `git/1` methods on a workspace, one module per command group (decision 0010).

mod status;

use clap::{Args, Subcommand};
use massorca_profile::Profile;

use crate::connect::connect;
use crate::cwd_path;
use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Subcommand)]
pub enum GitCommand {
    /// Shows the changed files: staged, unstaged, untracked and conflicted.
    Status(status::StatusArgs),
}

/// The workspace a git command acts on.
#[derive(Debug, Clone, Args)]
pub struct WorkspaceArg {
    /// Workspace id, name or path; defaults to the current directory.
    #[arg(short = 'w', long = "workspace", default_value = ".")]
    pub workspace: String,
}

impl WorkspaceArg {
    /// The selector sent to the service, with a relative path made absolute.
    pub fn selector(&self) -> Result<String, CliError> {
        cwd_path::selector(&self.workspace)
    }
}

/// Runs one `git` subcommand against the service of `profile`.
pub async fn execute(
    profile: &Profile,
    autostart: bool,
    command: GitCommand,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(profile, autostart).await?;
    match command {
        GitCommand::Status(args) => status::run(&client, args, output).await,
    }
}
