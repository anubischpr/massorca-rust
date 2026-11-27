//! `massorca git branches|branch-create|switch|branch-rename|branch-delete`.

use clap::Args;
use massorca_client::Client;
use massorca_protocol::methods::git::branch::{
    GitBranch, GitBranchCreateParams, GitBranchDeleteParams, GitBranchRenameParams,
    GitBranchSwitchParams, GitBranchesParams,
};

use super::WorkspaceArg;
use crate::cwd_path;
use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Args)]
pub struct BranchesArgs {
    /// Workspace id, name or path; defaults to the current directory. Not with `--project`.
    #[arg(short = 'w', long = "workspace", conflicts_with = "project")]
    pub workspace: Option<String>,
    /// Project id, name or path: lists the branches of the project's own checkout, with no workspace.
    #[arg(long, value_name = "PROJECT", conflicts_with = "workspace")]
    pub project: Option<String>,
}

#[derive(Debug, Args)]
pub struct CreateArgs {
    /// Name of the new branch.
    pub name: String,
    /// Where the branch starts; defaults to HEAD.
    #[arg(long)]
    pub start: Option<String>,
    /// Check the new branch out in the workspace.
    #[arg(long)]
    pub switch: bool,
    #[command(flatten)]
    pub workspace: WorkspaceArg,
}

#[derive(Debug, Args)]
pub struct SwitchArgs {
    /// A local branch.
    pub branch: String,
    #[command(flatten)]
    pub workspace: WorkspaceArg,
}

#[derive(Debug, Args)]
pub struct RenameArgs {
    pub from: String,
    pub to: String,
    #[command(flatten)]
    pub workspace: WorkspaceArg,
}

#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub branch: String,
    /// Delete the branch even when it is not merged.
    #[arg(long)]
    pub force: bool,
    #[command(flatten)]
    pub workspace: WorkspaceArg,
}

pub(super) async fn run_list(
    client: &Client,
    args: BranchesArgs,
    output: Output,
) -> Result<(), CliError> {
    let params = match args.project {
        Some(project) => GitBranchesParams {
            workspace: None,
            project: Some(project),
        },
        None => GitBranchesParams {
            workspace: Some(cwd_path::selector(
                args.workspace.as_deref().unwrap_or("."),
            )?),
            project: None,
        },
    };
    let listed = client.git_branches(&params).await?;
    let lines: Vec<String> = if listed.branches.is_empty() {
        vec!["no branches".to_owned()]
    } else {
        listed.branches.iter().map(branch_line).collect()
    };
    output.success(&listed, &lines);
    Ok(())
}

pub(super) async fn run_create(
    client: &Client,
    args: CreateArgs,
    output: Output,
) -> Result<(), CliError> {
    let created = client
        .git_branch_create(&GitBranchCreateParams {
            workspace: args.workspace.selector()?,
            name: args.name,
            start: args.start,
            switch: args.switch,
        })
        .await?;
    output.success(&created, &[format!("created {}", branch_line(&created))]);
    Ok(())
}

pub(super) async fn run_switch(
    client: &Client,
    args: SwitchArgs,
    output: Output,
) -> Result<(), CliError> {
    let switched = client
        .git_branch_switch(&GitBranchSwitchParams {
            workspace: args.workspace.selector()?,
            branch: args.branch,
        })
        .await?;
    output.success(&switched, &[format!("switched to {}", switched.branch)]);
    Ok(())
}

pub(super) async fn run_rename(
    client: &Client,
    args: RenameArgs,
    output: Output,
) -> Result<(), CliError> {
    let from = args.from.clone();
    let renamed = client
        .git_branch_rename(&GitBranchRenameParams {
            workspace: args.workspace.selector()?,
            from: args.from,
            to: args.to,
        })
        .await?;
    output.success(&renamed, &[format!("renamed {from} to {}", renamed.name)]);
    Ok(())
}

pub(super) async fn run_delete(
    client: &Client,
    args: DeleteArgs,
    output: Output,
) -> Result<(), CliError> {
    let deleted = client
        .git_branch_delete(&GitBranchDeleteParams {
            workspace: args.workspace.selector()?,
            branch: args.branch,
            force: args.force,
        })
        .await?;
    output.success(
        &deleted,
        &[format!(
            "deleted {} (was {})",
            deleted.branch,
            short(&deleted.commit)
        )],
    );
    Ok(())
}

/// `* name commit -> upstream [ahead N, behind M]`, with `*` on the current branch.
fn branch_line(branch: &GitBranch) -> String {
    let marker = if branch.current { '*' } else { ' ' };
    let mut line = format!("{marker} {} {}", branch.name, short(&branch.commit));
    if let Some(upstream) = &branch.upstream {
        line.push_str(&format!(" -> {upstream}"));
    }
    if branch.upstream_gone {
        line.push_str(" [gone]");
    } else if let (Some(ahead), Some(behind)) = (branch.ahead, branch.behind) {
        line.push_str(&format!(" [ahead {ahead}, behind {behind}]"));
    }
    if branch.checked_out_elsewhere {
        line.push_str(" (checked out elsewhere)");
    }
    line
}

fn short(commit: &str) -> &str {
    &commit[..commit.len().min(12)]
}
