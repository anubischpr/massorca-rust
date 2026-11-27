//! `massorca git remotes`: the hosting provider, owner and repository of each network remote (decision 0114).

use clap::Args;
use massorca_client::Client;
use massorca_protocol::methods::git::remotes::{
    GitRemotes, GitRemotesParams, Provider, RemoteIdentity,
};

use crate::cwd_path;
use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Args)]
pub struct RemotesArgs {
    /// Workspace id, name or path; defaults to the current directory. Not with --project.
    #[arg(short = 'w', long = "workspace", conflicts_with = "project")]
    pub workspace: Option<String>,
    /// The project (id, name or path) whose checkout to read. Not with --workspace.
    #[arg(long, value_name = "PROJECT", conflicts_with = "workspace")]
    pub project: Option<String>,
}

pub(super) async fn run(
    client: &Client,
    args: RemotesArgs,
    output: Output,
) -> Result<(), CliError> {
    let params = match args.project {
        Some(project) => GitRemotesParams {
            workspace: None,
            project: Some(cwd_path::selector(&project)?),
        },
        None => GitRemotesParams {
            workspace: Some(cwd_path::selector(
                args.workspace.as_deref().unwrap_or("."),
            )?),
            project: None,
        },
    };
    let remotes = client.git_remotes(&params).await?;
    output.success(&remotes, &lines(&remotes));
    Ok(())
}

/// Text form: one line per network remote, then `primary` and a fork's upstream when they apply.
pub(crate) fn lines(remotes: &GitRemotes) -> Vec<String> {
    if remotes.remotes.is_empty() {
        return vec!["no network remote".to_owned()];
    }
    remotes.remotes.iter().map(line).collect()
}

fn line(remote: &RemoteIdentity) -> String {
    let mut parts = vec![
        remote.remote.clone(),
        format!("{}/{}", remote.owner, remote.repo),
        format!("on {} ({})", remote.host, provider_name(remote.provider)),
    ];
    if remote.primary {
        parts.push("primary".to_owned());
    }
    if let Some(upstream) = &remote.fork_of {
        parts.push(format!("fork of {upstream}"));
    }
    parts.join("  ")
}

fn provider_name(provider: Provider) -> &'static str {
    match provider {
        Provider::Github => "github",
        Provider::Gitlab => "gitlab",
        Provider::Bitbucket => "bitbucket",
        Provider::AzureDevops => "azure_devops",
        Provider::Gitea => "gitea",
        Provider::Unknown => "unknown",
    }
}
