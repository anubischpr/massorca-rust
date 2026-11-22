//! `massorca git commit`.

use clap::Args;
use massorca_client::Client;
use massorca_protocol::methods::git::commit::GitCommitParams;

use super::WorkspaceArg;
use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Args)]
pub struct CommitArgs {
    /// The commit message; required unless `--amend` keeps HEAD's.
    #[arg(short = 'm', long)]
    pub message: Option<String>,
    /// Replace HEAD with a commit that also holds the staged changes.
    #[arg(long)]
    pub amend: bool,
    /// Amend even when HEAD is already on a remote-tracking branch.
    #[arg(long, requires = "amend")]
    pub allow_published: bool,
    #[command(flatten)]
    pub workspace: WorkspaceArg,
}

pub(super) async fn run(client: &Client, args: CommitArgs, output: Output) -> Result<(), CliError> {
    let committed = client
        .git_commit(&GitCommitParams {
            workspace: args.workspace.selector()?,
            message: args.message,
            amend: args.amend,
            allow_published: args.allow_published,
        })
        .await?;
    let short = &committed.id[..committed.id.len().min(12)];
    let line = match &committed.branch {
        Some(branch) => format!("[{branch} {short}] {}", committed.subject),
        None => format!("[detached {short}] {}", committed.subject),
    };
    output.success(&committed, &[line]);
    Ok(())
}
