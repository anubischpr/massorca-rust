//! `massorca terminal ...`: starting, listing and showing terminals through the service.

use std::collections::BTreeMap;

use clap::{Args, Subcommand};
use massorca_profile::Profile;
use massorca_protocol::methods::terminals::{
    ExitReason, ShellSpec, Terminal, TerminalCreateParams, TerminalExit, TerminalState,
};

use crate::connect::connect;
use crate::error::{CliError, exit};
use crate::output::Output;
use crate::workspace_text;

#[derive(Debug, Subcommand)]
pub enum TerminalCommand {
    /// Starts a shell, or a program with --shell, in a workspace. The terminal keeps running after this command exits.
    Create(TerminalCreateArgs),
    /// Lists live and exited terminals, or only those of one workspace.
    List(TerminalListArgs),
    /// Shows one terminal, selected by id.
    Show {
        /// Terminal id.
        terminal: String,
    },
}

#[derive(Debug, Args)]
pub struct TerminalCreateArgs {
    /// Workspace id, name or path.
    pub workspace: String,
    /// Program to run instead of the default shell.
    #[arg(long, value_name = "PROGRAM")]
    pub shell: Option<String>,
    /// Argument for --shell; repeat for several.
    #[arg(
        long = "arg",
        value_name = "ARG",
        allow_hyphen_values = true,
        requires = "shell"
    )]
    pub args: Vec<String>,
    /// Starting directory, relative to the workspace root or absolute.
    #[arg(long, value_name = "DIR")]
    pub cwd: Option<String>,
    /// Columns of the new terminal; give with --rows.
    #[arg(long, value_name = "N", requires = "rows")]
    pub cols: Option<u16>,
    /// Rows of the new terminal; give with --cols.
    #[arg(long, value_name = "N", requires = "cols")]
    pub rows: Option<u16>,
    /// Environment variable for the terminal, as KEY=VALUE; repeat for several.
    #[arg(long = "env", value_name = "KEY=VALUE")]
    pub env: Vec<String>,
    /// Title of the terminal.
    #[arg(long, value_name = "TITLE")]
    pub title: Option<String>,
}

#[derive(Debug, Args)]
pub struct TerminalListArgs {
    /// Only the terminals of this workspace (id, name or path).
    #[arg(long, value_name = "WORKSPACE")]
    pub workspace: Option<String>,
}

/// Runs one `terminal` subcommand against the service of `profile`.
pub async fn execute(
    profile: &Profile,
    autostart: bool,
    command: TerminalCommand,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(profile, autostart).await?;
    match command {
        TerminalCommand::Create(args) => {
            let params = create_params(args)?;
            let created = client.terminal_create(&params).await?;
            output.success(&created, &terminal_details(&created));
        }
        TerminalCommand::List(args) => {
            let list = client.terminal_list(args.workspace.as_deref()).await?;
            let lines = if list.terminals.is_empty() {
                vec!["no terminals".to_owned()]
            } else {
                terminal_table(&list.terminals)
            };
            output.success(&list, &lines);
        }
        TerminalCommand::Show { terminal } => {
            let shown = client.terminal_show(&terminal).await?;
            output.success(&shown, &terminal_details(&shown));
        }
    }
    Ok(())
}

/// The protocol params for `terminal create`. `--cwd` is passed on as typed: the service resolves a relative one against the workspace root.
fn create_params(args: TerminalCreateArgs) -> Result<TerminalCreateParams, CliError> {
    let shell = args.shell.map(|program| ShellSpec {
        program,
        args: args.args,
    });
    Ok(TerminalCreateParams {
        workspace: args.workspace,
        shell,
        cwd: args.cwd,
        env: parse_env(args.env)?,
        cols: args.cols,
        rows: args.rows,
        title: args.title,
        request_id: None,
    })
}

/// `KEY=VALUE` pairs. The message never repeats the entry, because a value may be a secret.
fn parse_env(entries: Vec<String>) -> Result<BTreeMap<String, String>, CliError> {
    entries
        .into_iter()
        .map(|entry| match entry.split_once('=') {
            Some((key, value)) if !key.is_empty() => Ok((key.to_owned(), value.to_owned())),
            _ => Err(CliError::new(
                "usage.invalid_env",
                "--env expects KEY=VALUE",
                exit::USAGE,
            )),
        })
        .collect()
}

/// The fields of one terminal, one per line.
pub fn terminal_details(terminal: &Terminal) -> Vec<String> {
    let mut lines = vec![
        format!("id: {}", terminal.id),
        format!("workspace: {}", terminal.workspace_id),
        format!("state: {}", state_text(terminal.state)),
    ];
    if let Some(exit) = &terminal.exit {
        lines.push(format!("exit: {}", exit_text(exit)));
    }
    lines.push(format!("shell: {}", terminal.shell));
    lines.push(format!("cwd: {}", terminal.cwd));
    lines.push(format!("size: {}x{}", terminal.cols, terminal.rows));
    if let Some(pid) = terminal.pid {
        lines.push(format!("pid: {pid}"));
    }
    if let Some(title) = &terminal.title {
        lines.push(format!("title: {title}"));
    }
    lines
}

fn terminal_table(terminals: &[Terminal]) -> Vec<String> {
    let rows = terminals
        .iter()
        .map(|terminal| {
            vec![
                terminal.id.clone(),
                state_text(terminal.state).to_owned(),
                terminal.shell.clone(),
                terminal.cwd.clone(),
            ]
        })
        .collect();
    workspace_text::table(&["ID", "STATE", "SHELL", "CWD"], rows)
}

pub fn state_text(state: TerminalState) -> &'static str {
    match state {
        TerminalState::Live => "live",
        TerminalState::Unverifiable => "unverifiable",
        TerminalState::Exited => "exited",
        TerminalState::Unknown => "unknown",
    }
}

/// How an exited terminal ended, for example `killed` or `exited, code 0`.
pub fn exit_text(exit: &TerminalExit) -> String {
    let reason = match exit.reason {
        ExitReason::Exited => "exited",
        ExitReason::Killed => "killed",
        ExitReason::ServiceStopped => "service stopped",
        ExitReason::Unknown => "unknown",
    };
    match (exit.code, &exit.signal) {
        (Some(code), _) => format!("{reason}, code {code}"),
        (None, Some(signal)) => format!("{reason}, signal {signal}"),
        (None, None) => reason.to_owned(),
    }
}
