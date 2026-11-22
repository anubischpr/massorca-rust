//! `massorca git watch`: prints what changes in a workspace until interrupted.

use clap::Args;
use std::future::Future;
use std::pin::Pin;

use massorca_client::{Client, ClientError, Subscription};
use massorca_protocol::methods::git::GitWorkspaceRef;
use massorca_protocol::methods::git::watch::GitWatchEvent;

use super::WorkspaceArg;
use crate::error::CliError;
use crate::output::Output;
use crate::stream_reconnect;
use crate::target::Target;

#[derive(Debug, Args)]
pub struct WatchArgs {
    #[command(flatten)]
    pub workspace: WorkspaceArg,
    /// Stop after this many change events (the first, `ready`, event is not counted).
    #[arg(long)]
    pub max_events: Option<u64>,
}

pub(super) async fn run(
    target: &Target,
    client: &Client,
    args: WatchArgs,
    output: Output,
) -> Result<(), CliError> {
    let params = GitWorkspaceRef {
        workspace: args.workspace.selector()?,
    };
    // The first attempt uses the command's connection; a refusal there fails the command before anything is printed.
    let mut stream = stream_reconnect::resilient(target, client, open, params);
    let mut seen = 0;
    while args.max_events.is_none_or(|max| seen < max) {
        let Some(item) = stream_reconnect::next_item(&mut stream, output).await else {
            break;
        };
        // Each event is its own line: a JSON document per line with `--json`. A reconnect opens a new watch, which
        // says `watching` again.
        let event: GitWatchEvent = serde_json::from_value(item?)
            .map_err(|error| CliError::from(ClientError::Protocol(error.to_string())))?;
        output.success(&event, &[line(&event)]);
        if !event.ready {
            seen += 1;
        }
    }
    Ok(())
}

fn open<'a>(
    client: &'a Client,
    params: &'a GitWorkspaceRef,
) -> Pin<Box<dyn Future<Output = Result<Subscription, ClientError>> + Send + 'a>> {
    Box::pin(client.git_watch(params))
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
