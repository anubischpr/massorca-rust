//! `massorca git rebase-base`: rebases a worktree's branch onto its base branch (decision 0181).

use clap::Args;
use massorca_client::Client;
use massorca_protocol::codes;
use massorca_protocol::methods::git::rebase::{GitRebaseBaseParams, GitRebased};

use super::WorkspaceArg;
use super::remote::{finish, short};
use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Args)]
pub struct RebaseBaseArgs {
    /// Fetch the base's remote-tracking branch first; a failed fetch refuses the rebase.
    #[arg(long)]
    pub refresh: bool,
    #[command(flatten)]
    pub workspace: WorkspaceArg,
}

pub(super) async fn run_rebase_base(
    client: &Client,
    args: RebaseBaseArgs,
    output: Output,
) -> Result<(), CliError> {
    let rebased = client
        .git_rebase_base(&GitRebaseBaseParams {
            workspace: args.workspace.selector()?,
            refresh: args.refresh,
        })
        .await?;
    let refusal = matches!(rebased, GitRebased::Conflict { .. }).then_some(codes::GIT_CONFLICTS);
    finish(output, &rebased, rebase_line(&rebased), refusal)
}

fn rebase_line(rebased: &GitRebased) -> String {
    match rebased {
        GitRebased::Rebased { head } => format!("rebased; HEAD is {}", short(head)),
        GitRebased::UpToDate => "already up to date".to_owned(),
        GitRebased::Conflict { paths } => format!(
            "stopped on conflicts in {}; the rebase was aborted and the worktree is as it was",
            paths.join(", ")
        ),
        GitRebased::Unknown => "rebased".to_owned(),
    }
}
