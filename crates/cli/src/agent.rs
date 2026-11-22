//! `massorca agent ...`: coding agents run by the service in workspaces (`docs/architecture/agents.md`).
//!
//! Each call connects on its own; a session keeps running after the command that started it exits.

use clap::{Args, Subcommand};
use massorca_client::Client;
use massorca_profile::Profile;
use massorca_protocol::methods::agents::{AgentSession, AgentSessionList, AgentStartParams};
use massorca_protocol::methods::terminals::TerminalState;

use crate::agent_text;
use crate::connect::connect;
use crate::error::{CliError, exit};
use crate::output::Output;
use crate::terminal_attach;

#[derive(Debug, Subcommand)]
pub enum AgentCommand {
    /// Lists the agent kinds: built in and declared in the profile, whether each is installed, and any problems.
    Kinds,
    /// Starts an agent in a workspace. The agent keeps running after this command exits.
    Start(AgentStartArgs),
    /// Lists agent sessions, newest first: every session, or those of one workspace.
    List(AgentListArgs),
    /// Shows one session's execution and activity, or every session's when none is named.
    Status {
        /// Session id, or a unique prefix of at least 8 characters.
        session: Option<String>,
    },
    /// Ends a session's run. The session stays listed as exited until forgotten.
    Stop {
        /// Session id, or a unique prefix of at least 8 characters.
        session: String,
        /// Skip the grace period and end the agent's processes at once.
        #[arg(long)]
        force: bool,
    },
    /// Forgets a session whose run has ended.
    Forget {
        /// Session id, or a unique prefix of at least 8 characters.
        session: String,
    },
    /// Connects this terminal to the session's run. Ctrl+] detaches; the agent keeps running.
    Attach {
        /// Session id, or a unique prefix of at least 8 characters.
        session: String,
    },
}

#[derive(Debug, Args)]
pub struct AgentStartArgs {
    /// Agent kind, for example claude.
    pub kind: String,
    /// Workspace id, name or path.
    #[arg(long, value_name = "WORKSPACE")]
    pub workspace: String,
    /// The first prompt, at most 8 KiB.
    #[arg(long, value_name = "TEXT")]
    pub prompt: Option<String>,
    /// Declare that the agent does not write; it does not hold the workspace's single writing slot.
    #[arg(long)]
    pub read_only: bool,
    /// Title of the session.
    #[arg(long, value_name = "TITLE")]
    pub title: Option<String>,
    /// Arguments passed to the agent after its own, given after `--`.
    #[arg(last = true, value_name = "ARGS")]
    pub args: Vec<String>,
}

#[derive(Debug, Args)]
pub struct AgentListArgs {
    /// Only the sessions of this workspace (id, name or path).
    #[arg(long, value_name = "WORKSPACE")]
    pub workspace: Option<String>,
}

/// Runs one `agent` subcommand against the service of `profile`.
pub async fn execute(
    profile: &Profile,
    autostart: bool,
    command: AgentCommand,
    output: Output,
    json: bool,
) -> Result<(), CliError> {
    if let AgentCommand::Attach { session } = &command {
        if json {
            return Err(CliError::new(
                "usage.invalid_arguments",
                "attach is interactive and has no --json form",
                exit::USAGE,
            ));
        }
        // Checked before connecting, so a refused attach never starts the service.
        terminal_attach::require_terminal()?;
        let client = connect(profile, autostart).await?;
        let terminal = live_terminal(&client, session).await?;
        return terminal_attach::attach(&client, &terminal).await;
    }

    let client = connect(profile, autostart).await?;
    match command {
        AgentCommand::Kinds => {
            let kinds = client.agent_kinds().await?;
            output.success(&kinds, &agent_text::kinds_lines(&kinds.kinds));
        }
        AgentCommand::Start(args) => {
            let started = client
                .agent_start(&AgentStartParams {
                    workspace: args.workspace,
                    kind: args.kind,
                    prompt: args.prompt,
                    args: args.args,
                    env: Default::default(),
                    read_only: args.read_only,
                    title: args.title,
                    cols: None,
                    rows: None,
                    request_id: None,
                })
                .await?;
            output.success(&started, &agent_text::session_details(&started));
        }
        AgentCommand::List(args) => {
            let list = client.agent_list(args.workspace.as_deref()).await?;
            output.success(&list, &agent_text::sessions_lines(&list.sessions));
        }
        AgentCommand::Status {
            session: Some(session),
        } => {
            let shown = client.agent_show(&session).await?;
            output.success(&shown, &agent_text::session_details(&shown));
        }
        AgentCommand::Status { session: None } => {
            let list: AgentSessionList = client.agent_list(None).await?;
            output.success(&list, &agent_text::sessions_lines(&list.sessions));
        }
        AgentCommand::Stop { session, force } => {
            let stopped = client.agent_stop(&session, force).await?;
            output.success(&stopped, &agent_text::session_details(&stopped));
        }
        AgentCommand::Forget { session } => {
            let forgotten = client.agent_forget(&session).await?;
            output.success(
                &forgotten,
                &[format!("forgot agent session {}", forgotten.id)],
            );
        }
        AgentCommand::Attach { .. } => unreachable!("attach returned above"),
    }
    Ok(())
}

/// The terminal of a session whose run is still there; an exited run is refused with how to read its last screen.
async fn live_terminal(client: &Client, selector: &str) -> Result<String, CliError> {
    let session: AgentSession = client.agent_show(selector).await?;
    let terminal = session.terminal_id.clone().unwrap_or_default();
    if session.execution.state == TerminalState::Exited || terminal.is_empty() {
        let hint = if terminal.is_empty() {
            String::new()
        } else {
            format!(
                "; `massorca terminal screen {terminal}` prints its last screen while the service keeps it"
            )
        };
        return Err(CliError::new(
            "agent.not_live",
            format!("the run of session {} has exited{hint}", session.id),
            exit::ERROR,
        ));
    }
    Ok(terminal)
}
