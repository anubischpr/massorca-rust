//! `massorca git log`.

use clap::Args;
use massorca_client::Client;
use massorca_protocol::methods::git::log::{GitLog, GitLogParams};

use super::WorkspaceArg;
use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Args)]
pub struct LogArgs {
    /// Where history starts; defaults to HEAD.
    pub revision: Option<String>,
    #[command(flatten)]
    pub workspace: WorkspaceArg,
    /// Commits per page (the service caps it at 500).
    #[arg(short = 'n', long)]
    pub limit: Option<u32>,
    /// Continue after a previous page; the cursor is printed at its end.
    #[arg(long, conflicts_with = "revision")]
    pub cursor: Option<String>,
}

pub(super) async fn run(client: &Client, args: LogArgs, output: Output) -> Result<(), CliError> {
    let log = client
        .git_log(&GitLogParams {
            workspace: args.workspace.selector()?,
            revision: args.revision,
            limit: args.limit,
            cursor: args.cursor,
        })
        .await?;
    output.success(&log, &lines(&log));
    Ok(())
}

/// One line per commit: short id, author, subject; then how to read the next page.
fn lines(log: &GitLog) -> Vec<String> {
    if log.commits.is_empty() {
        return vec!["no commits".to_owned()];
    }
    let mut lines: Vec<String> = log
        .commits
        .iter()
        .map(|commit| {
            let short = &commit.id[..commit.id.len().min(12)];
            format!("{short} {} {}", commit.author.name, commit.subject)
        })
        .collect();
    if let Some(cursor) = &log.next_cursor {
        lines.push(format!("(more: --cursor {cursor})"));
    }
    lines
}
