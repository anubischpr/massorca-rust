//! `massorca git stage|unstage|discard`.

use clap::Args;
use massorca_client::Client;
use massorca_protocol::methods::git::index::{GitDiscardParams, GitDiscarded, GitPathsParams};

use super::WorkspaceArg;
use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Args)]
pub struct PathsArgs {
    /// Files relative to the workspace root.
    #[arg(required_unless_present = "all", conflicts_with = "all")]
    pub paths: Vec<String>,
    /// Every changed file.
    #[arg(long)]
    pub all: bool,
    #[command(flatten)]
    pub workspace: WorkspaceArg,
}

#[derive(Debug, Args)]
pub struct DiscardArgs {
    #[command(flatten)]
    pub target: PathsArgs,
    /// Also delete untracked files, which git has no copy of.
    #[arg(long)]
    pub delete_untracked: bool,
    /// Also drop staged changes, restoring files from HEAD.
    #[arg(long)]
    pub include_staged: bool,
    /// Print what would be discarded and change nothing.
    #[arg(long)]
    pub dry_run: bool,
}

impl PathsArgs {
    fn params(self) -> Result<GitPathsParams, CliError> {
        Ok(GitPathsParams {
            workspace: self.workspace.selector()?,
            paths: self.paths,
            all: self.all,
        })
    }
}

pub(super) async fn run_stage(
    client: &Client,
    args: PathsArgs,
    output: Output,
) -> Result<(), CliError> {
    client.git_stage(&args.params()?).await?;
    output.success(&(), &["staged".to_owned()]);
    Ok(())
}

pub(super) async fn run_unstage(
    client: &Client,
    args: PathsArgs,
    output: Output,
) -> Result<(), CliError> {
    client.git_unstage(&args.params()?).await?;
    output.success(&(), &["unstaged".to_owned()]);
    Ok(())
}

pub(super) async fn run_discard(
    client: &Client,
    args: DiscardArgs,
    output: Output,
) -> Result<(), CliError> {
    let target = args.target.params()?;
    let discarded = client
        .git_discard(&GitDiscardParams {
            workspace: target.workspace,
            paths: target.paths,
            all: target.all,
            delete_untracked: args.delete_untracked,
            include_staged: args.include_staged,
            dry_run: args.dry_run,
        })
        .await?;
    output.success(&discarded, &discard_lines(&discarded));
    Ok(())
}

fn discard_lines(discarded: &GitDiscarded) -> Vec<String> {
    let verb =
        |done: &'static str, planned: &'static str| if discarded.dry_run { planned } else { done };
    let mut lines = Vec::new();
    for path in &discarded.restored_from_index {
        lines.push(format!(
            "{} {path} (unstaged edits)",
            verb("discarded", "would discard")
        ));
    }
    for path in &discarded.restored_from_head {
        lines.push(format!(
            "{} {path} (staged and unstaged changes)",
            verb("discarded", "would discard")
        ));
    }
    for path in &discarded.deleted {
        lines.push(format!("{} {path}", verb("deleted", "would delete")));
    }
    if lines.is_empty() {
        lines.push("nothing to discard".to_owned());
    }
    lines
}
