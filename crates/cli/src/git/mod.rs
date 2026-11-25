//! `massorca git ...`: the `git/1` methods on a workspace, one module per command group (decision 0010).

mod branch;
mod commit;
mod diff;
mod index;
mod log;
mod remote;
mod show_file;
mod status;
mod watch;

use crate::target::Target;
use clap::{Args, Subcommand};

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
    /// Fetches from a remote, pruning branches deleted there.
    Fetch(remote::FetchArgs),
    /// Brings the branch up to date with its upstream; never merges unless `--mode` says so.
    Pull(remote::PullArgs),
    /// Pushes the branch to its upstream, creating it on the remote if needed. Never forces.
    Push(remote::PushArgs),
    /// Aborts the merge, rebase, cherry-pick or revert in progress.
    Abort(WorkspaceArg),
    /// Prints what changes in a workspace (working tree, index, HEAD, refs) until interrupted.
    Watch(watch::WatchArgs),
    /// Lists the files a comparison changes, with line counts.
    Diff(diff::DiffArgs),
    /// Shows the history, newest first, a page at a time.
    Log(log::LogArgs),
    /// Shows the patch of one file.
    Patch(diff::PatchArgs),
    /// Shows one file's bytes on the working tree, the index or a revision; `--out` writes them.
    ShowFile(show_file::ShowFileArgs),
    /// Lists the local branches, the current one first.
    Branches(branch::BranchesArgs),
    /// Creates a branch with no upstream, at HEAD or `--start`.
    BranchCreate(branch::CreateArgs),
    /// Checks out a local branch; refuses when local changes would be overwritten.
    Switch(branch::SwitchArgs),
    /// Renames a branch; never overwrites another.
    BranchRename(branch::RenameArgs),
    /// Deletes a merged branch, or any with `--force`; prints the commit it pointed at.
    BranchDelete(branch::DeleteArgs),
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

/// Runs one `git` subcommand against the service `target` names.
pub async fn execute(
    target: &Target,
    autostart: bool,
    command: GitCommand,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(target, autostart).await?;
    match command {
        GitCommand::Status(args) => status::run(&client, args, output).await,
        GitCommand::Stage(args) => index::run_stage(&client, args, output).await,
        GitCommand::Unstage(args) => index::run_unstage(&client, args, output).await,
        GitCommand::Discard(args) => index::run_discard(&client, args, output).await,
        GitCommand::Commit(args) => commit::run(&client, args, output).await,
        GitCommand::Fetch(args) => remote::run_fetch(&client, args, output).await,
        GitCommand::Pull(args) => remote::run_pull(&client, args, output).await,
        GitCommand::Push(args) => remote::run_push(&client, args, output).await,
        GitCommand::Abort(workspace) => remote::run_abort(&client, workspace, output).await,
        GitCommand::Watch(args) => watch::run(target, &client, args, output).await,
        GitCommand::Diff(args) => diff::run_list(&client, args, output).await,
        GitCommand::Log(args) => log::run(&client, args, output).await,
        GitCommand::Patch(args) => diff::run_patch(&client, args, output).await,
        GitCommand::ShowFile(args) => show_file::run(&client, args, output).await,
        GitCommand::Branches(args) => branch::run_list(&client, args, output).await,
        GitCommand::BranchCreate(args) => branch::run_create(&client, args, output).await,
        GitCommand::Switch(args) => branch::run_switch(&client, args, output).await,
        GitCommand::BranchRename(args) => branch::run_rename(&client, args, output).await,
        GitCommand::BranchDelete(args) => branch::run_delete(&client, args, output).await,
    }
}
