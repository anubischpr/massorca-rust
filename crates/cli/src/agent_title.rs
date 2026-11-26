//! `massorca agent title`: names a chat from its first prompt and reply, by one run of the kind's one-shot form (decision
//! 0167). The service runs it; this command prints the slug alone, or the result document with `--json`.

use clap::Args;
use massorca_client::Client;
use massorca_protocol::methods::agents::GenerateTitleParams;

use crate::error::CliError;
use crate::output::Output;

/// Params of `massorca agent title`.
#[derive(Debug, Args)]
pub struct AgentTitleArgs {
    /// Agent kind, for example claude. The kind must declare a one-shot form.
    #[arg(long)]
    pub kind: String,
    /// The chat's first prompt.
    #[arg(long)]
    pub prompt: String,
    /// The agent's first reply, when it has answered.
    #[arg(long)]
    pub reply: Option<String>,
    /// The model to name the chat with, for a kind that reuses its model flag for the run.
    #[arg(long)]
    pub model: Option<String>,
    /// The account to run with. Without it, the kind's active account runs.
    #[arg(long)]
    pub account: Option<String>,
}

/// Runs `agent title`: asks the service for the slug and prints it.
pub async fn execute(
    client: &Client,
    args: AgentTitleArgs,
    output: Output,
) -> Result<(), CliError> {
    let named = client
        .agent_generate_title(&GenerateTitleParams {
            kind: args.kind,
            account: args.account,
            model: args.model,
            prompt: args.prompt,
            reply: args.reply,
        })
        .await?;
    output.success(&named, std::slice::from_ref(&named.slug));
    Ok(())
}
