//! `massorca terminal ...`: starting, reading, typing into, resizing and stopping terminals through the service.
//!
//! `attach` is interactive and lives in `terminal_attach`; `screen` reads one snapshot and prints it as text.

use std::collections::BTreeMap;
use std::num::NonZeroU16;
use std::time::Duration;

use clap::{Args, Subcommand};
use massorca_client::{AttachUpdate, Client, ClientError, TerminalAttachment};
use massorca_core_terminal::screen::SCROLLBACK_ROWS;
use massorca_profile::Profile;
use massorca_protocol::methods::terminals::{
    ExitReason, ShellSpec, Terminal, TerminalAttachParams, TerminalCreateParams, TerminalExit,
    TerminalResizeParams, TerminalState,
};
use serde::Serialize;

use crate::connect::connect;
use crate::error::{CliError, exit};
use crate::output::Output;
use crate::terminal_attach;
use crate::workspace_text;

/// How long `screen` waits for the service to send the terminal's snapshot.
const SNAPSHOT_WAIT: Duration = Duration::from_secs(5);

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
    /// Types text into a terminal. The arguments are joined with spaces; Enter follows unless --no-enter.
    Send(TerminalSendArgs),
    /// Prints the terminal's visible screen as text.
    Screen(TerminalScreenArgs),
    /// Connects this terminal to the terminal's input and output. Ctrl+] detaches; the terminal keeps running.
    Attach {
        /// Terminal id.
        terminal: String,
    },
    /// Sets the terminal's size.
    Resize(TerminalResizeArgs),
    /// Ends the terminal's processes. The terminal stays listed as exited until closed.
    Kill(TerminalKillArgs),
    /// Ends the terminal's processes and forgets the terminal.
    Close {
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

#[derive(Debug, Args)]
pub struct TerminalSendArgs {
    /// Terminal id.
    pub terminal: String,
    /// Text to type, joined with single spaces. Use `--` before text that starts with a dash.
    #[arg(required = true, num_args = 1..)]
    pub text: Vec<String>,
    /// Do not press Enter after the text.
    #[arg(long)]
    pub no_enter: bool,
}

#[derive(Debug, Args)]
pub struct TerminalScreenArgs {
    /// Terminal id.
    pub terminal: String,
    /// Print the history above the visible screen first.
    #[arg(long)]
    pub scrollback: bool,
}

#[derive(Debug, Args)]
pub struct TerminalResizeArgs {
    /// Terminal id.
    pub terminal: String,
    /// Columns.
    #[arg(long)]
    pub cols: u16,
    /// Rows.
    #[arg(long)]
    pub rows: u16,
}

#[derive(Debug, Args)]
pub struct TerminalKillArgs {
    /// Terminal id.
    pub terminal: String,
    /// Skip the grace period and end the processes at once.
    #[arg(long)]
    pub force: bool,
}

/// The `screen` result: the terminal and its visible rows as plain text.
#[derive(Debug, Serialize)]
struct ScreenDocument {
    terminal: Terminal,
    lines: Vec<String>,
}

/// Runs one `terminal` subcommand against the service of `profile`.
pub async fn execute(
    profile: &Profile,
    autostart: bool,
    command: TerminalCommand,
    output: Output,
    json: bool,
) -> Result<(), CliError> {
    if let TerminalCommand::Attach { terminal } = &command {
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
        return terminal_attach::attach(&client, terminal).await;
    }

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
        TerminalCommand::Send(args) => {
            let bytes = typed_bytes(&args.text, args.no_enter);
            client.terminal_input(&args.terminal, &bytes).await?;
            output.success(&(), &[]);
        }
        TerminalCommand::Screen(args) => {
            let document = read_screen(&client, &args.terminal, args.scrollback).await?;
            output.success(&document, &document.lines);
        }
        TerminalCommand::Resize(args) => {
            let resized = client
                .terminal_resize(&TerminalResizeParams {
                    terminal: args.terminal,
                    cols: args.cols,
                    rows: args.rows,
                })
                .await?;
            output.success(&resized, &terminal_details(&resized));
        }
        TerminalCommand::Kill(args) => {
            let killed = client.terminal_kill(&args.terminal, args.force).await?;
            output.success(&killed, &terminal_details(&killed));
        }
        TerminalCommand::Close { terminal } => {
            let closed = client.terminal_close(&terminal).await?;
            output.success(&closed, &[format!("closed terminal {}", closed.id)]);
        }
        TerminalCommand::Attach { .. } => unreachable!("attach returned above"),
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

/// The bytes `send` types: the text, then a carriage return (Enter for ConPTY and Unix line disciplines) unless `no_enter`.
fn typed_bytes(text: &[String], no_enter: bool) -> Vec<u8> {
    let mut bytes = text.join(" ").into_bytes();
    if !no_enter {
        bytes.push(b'\r');
    }
    bytes
}

/// The terminal's current screen, read from the snapshot that its attach sends first.
async fn read_screen(
    client: &Client,
    terminal: &str,
    scrollback: bool,
) -> Result<ScreenDocument, CliError> {
    let mut attachment = client
        .terminal_attach(&TerminalAttachParams {
            terminal: terminal.to_owned(),
            since: None,
            cols: None,
            rows: None,
        })
        .await?;
    let snapshot = tokio::time::timeout(SNAPSHOT_WAIT, take_snapshot(&mut attachment))
        .await
        .map_err(|_| {
            CliError::new(
                "terminal.snapshot_timeout",
                "the terminal did not send its screen in time",
                exit::ERROR,
            )
        })??;
    drop(attachment);
    Ok(ScreenDocument {
        lines: screen_lines(&snapshot, scrollback),
        terminal: snapshot.terminal,
    })
}

/// A snapshot as the attach stream delivered it.
struct Snapshot {
    terminal: Terminal,
    cols: u16,
    rows: u16,
    bytes: Vec<u8>,
}

async fn take_snapshot(attachment: &mut TerminalAttachment) -> Result<Snapshot, CliError> {
    let mut attached = None;
    loop {
        let Some(update) = attachment.next().await else {
            return Err(ClientError::Disconnected.into());
        };
        match update? {
            AttachUpdate::Attached { terminal, .. } => attached = Some(terminal),
            AttachUpdate::Snapshot {
                cols, rows, bytes, ..
            } => {
                let terminal = attached.ok_or_else(|| {
                    CliError::from(ClientError::Protocol("snapshot before attached".to_owned()))
                })?;
                return Ok(Snapshot {
                    terminal,
                    cols,
                    rows,
                    bytes,
                });
            }
            AttachUpdate::Output { .. }
            | AttachUpdate::Resized { .. }
            | AttachUpdate::Exited { .. } => {}
        }
    }
}

/// Replays the snapshot into a parser of its size and reads the rows as text, with trailing spaces and blank rows removed.
/// The parser is `vt100` directly: `core_terminal::screen::Screen` has no text accessor.
fn screen_lines(snapshot: &Snapshot, scrollback: bool) -> Vec<String> {
    let rows = NonZeroU16::new(snapshot.rows).unwrap_or(NonZeroU16::MIN);
    let cols = NonZeroU16::new(snapshot.cols).unwrap_or(NonZeroU16::MIN);
    let mut parser = vt100::Parser::new(rows, cols, SCROLLBACK_ROWS);
    parser.process(&snapshot.bytes);
    let screen = parser.screen_mut();
    let width = screen.size().1.get();
    let mut lines = Vec::new();
    if scrollback {
        // With the view `history - index` rows up, its first row is history row `index`, oldest first.
        let history = screen.scrollback_len();
        for index in 0..history {
            screen.set_scrollback(history - index);
            lines.extend(screen.rows(0, width).next());
        }
        screen.set_scrollback(0);
    }
    lines.extend(screen.rows(0, width));
    let mut lines: Vec<String> = lines
        .into_iter()
        .map(|line| line.trim_end().to_owned())
        .collect();
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
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
