//! `massorca agent ...`: coding agents run by the service in workspaces (`docs/architecture/agents.md`).
//!
//! Each call connects on its own; a session keeps running after the command that started it exits.

use std::time::Duration;

use clap::{Args, Subcommand, ValueEnum};
use massorca_client::Client;
use massorca_protocol::methods::agents::{
    ActivityState, AgentPromptParams, AgentSession, AgentSessionList, AgentStartParams,
    AgentWatchEvent, AgentWatchParams,
};
use massorca_protocol::methods::terminals::TerminalState;

use crate::agent_text;
use crate::connect::connect;
use crate::error::{CliError, exit};
use crate::output::Output;
use crate::target::Target;
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
    Status(AgentStatusArgs),
    /// Ends a session's run. The session stays listed as exited until forgotten.
    Stop {
        /// Session id, or a unique prefix of at least 8 characters.
        session: String,
        /// Skip the grace period and end the agent's processes at once.
        #[arg(long)]
        force: bool,
    },
    /// Types a prompt into a session's terminal once its agent is idle or done, then submits it.
    Prompt(AgentPromptArgs),
    /// Starts a new run of an exited session with the agent's resume arguments; the session keeps its id.
    Resume {
        /// Session id, or a unique prefix of at least 8 characters.
        session: String,
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
    /// Reports an agent run's status to the service; agents' hooks call it. Always exits 0 within 5 s.
    Hook(AgentHookArgs),
}

#[derive(Debug, Args)]
pub struct AgentStatusArgs {
    /// Session id, or a unique prefix of at least 8 characters.
    pub session: Option<String>,
    /// Print every change until interrupted.
    #[arg(long, conflicts_with = "until")]
    pub watch: bool,
    /// Wait until the session reaches this activity, or until its run has exited.
    #[arg(long, value_enum, value_name = "STATE", requires = "session")]
    pub until: Option<UntilState>,
    /// Give up waiting for `--until` after this many seconds and exit non-zero.
    #[arg(long, value_name = "SECS", requires = "until")]
    pub timeout: Option<u64>,
}

/// What `status --until` waits for: an activity, or the run's exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum UntilState {
    Working,
    Waiting,
    Idle,
    Done,
    Exited,
}

impl UntilState {
    fn name(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Waiting => "waiting",
            Self::Idle => "idle",
            Self::Done => "done",
            Self::Exited => "exited",
        }
    }

    fn reached_by(self, session: &AgentSession) -> bool {
        let activity = session.activity.as_ref().map(|activity| activity.state);
        match self {
            Self::Exited => session.execution.state == TerminalState::Exited,
            Self::Working => activity == Some(ActivityState::Working),
            Self::Waiting => activity == Some(ActivityState::Waiting),
            Self::Idle => activity == Some(ActivityState::Idle),
            Self::Done => activity == Some(ActivityState::Done),
        }
    }
}

/// The hook's arguments. Every form parses, so a hook never fails the agent on its command line (0053): a missing
/// event is reported on stderr and words after the payload are ignored.
#[derive(Debug, Args)]
pub struct AgentHookArgs {
    /// The hook event, mapped to an activity by the kind's declaration.
    #[arg(allow_hyphen_values = true)]
    pub event: Option<String>,
    /// The hook's payload, at most 64 KiB. Without it, stdin is read when it is not a terminal.
    #[arg(allow_hyphen_values = true)]
    pub payload: Option<String>,
    /// Anything after the payload; ignored.
    #[arg(hide = true, num_args = 0.., allow_hyphen_values = true, trailing_var_arg = true)]
    pub ignored: Vec<String>,
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
pub struct AgentPromptArgs {
    /// Session id, or a unique prefix of at least 8 characters.
    pub session: String,
    /// The text to type, at most 8 KiB; the agent's submit key follows it.
    #[arg(allow_hyphen_values = true)]
    pub text: String,
    /// Type the text even while the agent works or waits, for example to answer its question.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct AgentListArgs {
    /// Only the sessions of this workspace (id, name or path).
    #[arg(long, value_name = "WORKSPACE")]
    pub workspace: Option<String>,
}

/// Runs one `agent` subcommand against the service `target` names.
pub async fn execute(
    target: &Target,
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
        let client = connect(target, autostart).await?;
        let terminal = live_terminal(&client, session).await?;
        return terminal_attach::attach(&client, &terminal).await;
    }

    let client = connect(target, autostart).await?;
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
        AgentCommand::Status(AgentStatusArgs {
            session: Some(session),
            until: Some(until),
            timeout,
            ..
        }) => {
            let reached = wait_until(&client, &session, until, timeout).await?;
            output.success(&reached, &agent_text::session_details(&reached));
        }
        AgentCommand::Status(AgentStatusArgs {
            session,
            watch: true,
            ..
        }) => watch(&client, session, output).await?,
        AgentCommand::Status(AgentStatusArgs {
            session: Some(session),
            ..
        }) => {
            let shown = client.agent_show(&session).await?;
            output.success(&shown, &agent_text::session_details(&shown));
        }
        AgentCommand::Status(AgentStatusArgs { session: None, .. }) => {
            let list: AgentSessionList = client.agent_list(None).await?;
            output.success(&list, &agent_text::sessions_lines(&list.sessions));
        }
        AgentCommand::Stop { session, force } => {
            let stopped = client.agent_stop(&session, force).await?;
            output.success(&stopped, &agent_text::session_details(&stopped));
        }
        AgentCommand::Prompt(args) => {
            let receipt = client
                .agent_prompt(&AgentPromptParams {
                    session: args.session,
                    text: args.text,
                    force: args.force,
                })
                .await?;
            let lines = [format!("handed to the terminal: {} bytes", receipt.bytes)];
            output.success(&receipt, &lines);
        }
        AgentCommand::Resume { session } => {
            let resumed = client.agent_resume(&session).await?;
            output.success(&resumed, &agent_text::session_details(&resumed));
        }
        AgentCommand::Forget { session } => {
            let forgotten = client.agent_forget(&session).await?;
            output.success(
                &forgotten,
                &[format!("forgot agent session {}", forgotten.id)],
            );
        }
        AgentCommand::Attach { .. } => unreachable!("attach returned above"),
        AgentCommand::Hook(_) => unreachable!("hook runs without a connection, from app"),
    }
    Ok(())
}

/// Waits on the watch stream until the session reaches `until`; returns its record then.
async fn wait_until(
    client: &Client,
    selector: &str,
    until: UntilState,
    timeout: Option<u64>,
) -> Result<AgentSession, CliError> {
    let mut stream = client
        .agent_watch(&AgentWatchParams {
            workspace: None,
            session: Some(selector.to_owned()),
        })
        .await?;
    let wait = async {
        while let Some(item) = stream.next_as::<AgentWatchEvent>().await {
            let sessions = match item? {
                AgentWatchEvent::Snapshot { sessions } => sessions,
                AgentWatchEvent::Changed { session } => vec![*session],
                AgentWatchEvent::Unknown => Vec::new(),
            };
            if let Some(reached) = sessions
                .into_iter()
                .find(|session| until.reached_by(session))
            {
                return Ok(reached);
            }
        }
        Err(CliError::new(
            "agent.watch_ended",
            format!(
                "the status stream ended before session {selector} reached {}",
                until.name()
            ),
            exit::ERROR,
        ))
    };
    match timeout {
        None => wait.await,
        Some(secs) => tokio::time::timeout(Duration::from_secs(secs), wait)
            .await
            .unwrap_or_else(|_| {
                Err(CliError::new(
                    "agent.status_timeout",
                    format!(
                        "session {selector} did not reach {} within {secs} s",
                        until.name()
                    ),
                    exit::ERROR,
                ))
            }),
    }
}

/// Prints the selected sessions, then every change, until the stream ends or the command is interrupted. With
/// `--json` each item is its own `{"ok":true,"result":...}` line.
async fn watch(client: &Client, session: Option<String>, output: Output) -> Result<(), CliError> {
    let mut stream = client
        .agent_watch(&AgentWatchParams {
            workspace: None,
            session,
        })
        .await?;
    while let Some(item) = stream.next_as::<AgentWatchEvent>().await {
        let item = item?;
        let lines = match &item {
            AgentWatchEvent::Snapshot { sessions } => agent_text::sessions_lines(sessions),
            AgentWatchEvent::Changed { session } => vec![agent_text::change_line(session)],
            AgentWatchEvent::Unknown => continue,
        };
        output.success(&item, &lines);
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
