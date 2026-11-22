//! `massorca git fetch|pull|push|abort`.

use clap::{Args, ValueEnum};
use massorca_client::Client;
use massorca_protocol::methods::git::GitWorkspaceRef;
use massorca_protocol::methods::git::remote::{
    GitFetchParams, GitPullParams, GitPulled, GitPushParams, GitPushed, PullMode, PushOutcome,
};

use super::WorkspaceArg;
use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Args)]
pub struct FetchArgs {
    /// The remote; defaults to the current branch's, else `origin`.
    pub remote: Option<String>,
    /// Fetch every remote.
    #[arg(long, conflicts_with = "remote")]
    pub all: bool,
    #[command(flatten)]
    pub workspace: WorkspaceArg,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PullModeArg {
    /// Only move the branch when no merge is needed (the default).
    FfOnly,
    Merge,
    Rebase,
}

#[derive(Debug, Args)]
pub struct PullArgs {
    #[arg(long, value_enum, default_value = "ff-only")]
    pub mode: PullModeArg,
    #[command(flatten)]
    pub workspace: WorkspaceArg,
}

#[derive(Debug, Args)]
pub struct PushArgs {
    /// The remote for a branch without an upstream; defaults to `origin`.
    pub remote: Option<String>,
    /// Overwrite the remote branch only if it still points at this commit.
    #[arg(long, value_name = "COMMIT")]
    pub force_with_lease: Option<String>,
    #[command(flatten)]
    pub workspace: WorkspaceArg,
}

pub(super) async fn run_fetch(
    client: &Client,
    args: FetchArgs,
    output: Output,
) -> Result<(), CliError> {
    let fetched = client
        .git_fetch(&GitFetchParams {
            workspace: args.workspace.selector()?,
            remote: args.remote,
            all: args.all,
            timeout_ms: None,
        })
        .await?;
    let mut lines: Vec<String> = fetched
        .updated
        .iter()
        .map(|change| match (&change.old, &change.new) {
            (None, Some(_)) => format!("new      {}", change.name),
            (Some(_), None) => format!("pruned   {}", change.name),
            _ => format!("updated  {}", change.name),
        })
        .collect();
    if lines.is_empty() {
        lines.push("already up to date".to_owned());
    }
    output.success(&fetched, &lines);
    Ok(())
}

pub(super) async fn run_pull(
    client: &Client,
    args: PullArgs,
    output: Output,
) -> Result<(), CliError> {
    let mode = match args.mode {
        PullModeArg::FfOnly => PullMode::FfOnly,
        PullModeArg::Merge => PullMode::Merge,
        PullModeArg::Rebase => PullMode::Rebase,
    };
    let pulled = client
        .git_pull(&GitPullParams {
            workspace: args.workspace.selector()?,
            mode,
            timeout_ms: None,
        })
        .await?;
    output.success(&pulled, &[pull_line(&pulled)]);
    Ok(())
}

pub(super) async fn run_push(
    client: &Client,
    args: PushArgs,
    output: Output,
) -> Result<(), CliError> {
    let pushed = client
        .git_push(&GitPushParams {
            workspace: args.workspace.selector()?,
            remote: args.remote,
            force_with_lease: args.force_with_lease,
            timeout_ms: None,
        })
        .await?;
    output.success(&pushed, &[push_line(&pushed)]);
    Ok(())
}

pub(super) async fn run_abort(
    client: &Client,
    workspace: WorkspaceArg,
    output: Output,
) -> Result<(), CliError> {
    let aborted = client
        .git_abort(&GitWorkspaceRef {
            workspace: workspace.selector()?,
        })
        .await?;
    output.success(
        &aborted,
        &[format!("aborted the {:?}", aborted.operation).to_lowercase()],
    );
    Ok(())
}

fn short(id: &str) -> &str {
    &id[..id.len().min(12)]
}

fn pull_line(pulled: &GitPulled) -> String {
    match pulled {
        GitPulled::UpToDate => "already up to date".to_owned(),
        GitPulled::FastForwarded { from, to } => {
            format!("fast-forwarded {}..{}", short(from), short(to))
        }
        GitPulled::Merged { commit } => format!("merged as {}", short(commit)),
        GitPulled::Rebased { head } => format!("rebased; HEAD is {}", short(head)),
        GitPulled::Diverged { ahead, behind } => format!(
            "not pulled: {ahead} local and {behind} remote commits diverge; use --mode merge or --mode rebase"
        ),
        GitPulled::Conflicts { paths } => format!(
            "stopped on conflicts in {}; resolve them, or run `massorca git abort`",
            paths.join(", ")
        ),
        GitPulled::Unknown => "pulled".to_owned(),
    }
}

fn push_line(pushed: &GitPushed) -> String {
    let outcome = match &pushed.outcome {
        PushOutcome::Created => "created".to_owned(),
        PushOutcome::FastForward => "updated".to_owned(),
        PushOutcome::Forced => "overwritten (lease held)".to_owned(),
        PushOutcome::UpToDate => "already up to date".to_owned(),
        PushOutcome::Rejected { reason } => format!("rejected ({reason})"),
        PushOutcome::Unknown => "pushed".to_owned(),
    };
    let upstream = if pushed.upstream_set {
        ", upstream set"
    } else {
        ""
    };
    format!("{}: {outcome}{upstream}", pushed.remote_branch)
}
