//! `massorca git ...`: the `git/1` methods on a workspace, one module per command group (decision 0010).

mod commit;
mod diff;
mod index;
mod log;
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
    /// Stages files, or every change with `--all`.
    Stage(index::PathsArgs),
    /// Unstages files, or everything with `--all`; the working files are kept.
    Unstage(index::PathsArgs),
    /// Discards changes to files. Refuses to delete untracked files or drop staged changes unless told to.
    Discard(index::DiscardArgs),
    /// Commits the staged changes, or amends HEAD; hooks run.
    Commit(commit::CommitArgs),
    /// Lists the files a comparison changes, with line counts.
    Diff(diff::DiffArgs),
    /// Shows the history, newest first, a page at a time.
    Log(log::LogArgs),
    /// Shows the patch of one file.
    Patch(diff::PatchArgs),
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
        GitCommand::Stage(args) => index::run_stage(&client, args, output).await,
        GitCommand::Unstage(args) => index::run_unstage(&client, args, output).await,
        GitCommand::Discard(args) => index::run_discard(&client, args, output).await,
        GitCommand::Commit(args) => commit::run(&client, args, output).await,
        GitCommand::Diff(args) => diff::run_list(&client, args, output).await,
        GitCommand::Log(args) => log::run(&client, args, output).await,
        GitCommand::Patch(args) => diff::run_patch(&client, args, output).await,
    }
}
