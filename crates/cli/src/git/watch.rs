//! `massorca git watch`: prints what changes in a workspace until interrupted.

use clap::Args;
use massorca_client::Client;
use massorca_protocol::methods::git::GitWorkspaceRef;
use massorca_protocol::methods::git::watch::GitWatchEvent;

use super::WorkspaceArg;
use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Args)]
pub struct WatchArgs {
    #[command(flatten)]
    pub workspace: WorkspaceArg,
    /// Stop after this many change events (the first, `ready`, event is not counted).
    #[arg(long)]
    pub max_events: Option<u64>,
}

pub(super) async fn run(client: &Client, args: WatchArgs, output: Output) -> Result<(), CliError> {
    let mut stream = client
        .git_watch(&GitWorkspaceRef {
            workspace: args.workspace.selector()?,
        })
        .await?;
    let mut seen = 0;
    while args.max_events.is_none_or(|max| seen < max) {
        let Some(item) = stream.next_as::<GitWatchEvent>().await else {
            break;
        };
        // Each event is its own line: a JSON document per line with `--json`.
        let event = item?;
        output.success(&event, &[line(&event)]);
        if !event.ready {
            seen += 1;
        }
    }
    Ok(())
}

fn line(event: &GitWatchEvent) -> String {
    if event.ready {
        return match &event.reason {
            Some(reason) => format!("watching (working-tree edits not observed: {reason})"),
            None => "watching".to_owned(),
        };
    }
    let kinds: Vec<String> = event
        .kinds
        .iter()
        .map(|kind| format!("{kind:?}").to_lowercase())
        .collect();
    format!("changed: {}", kinds.join(", "))
}
