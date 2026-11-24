//! `massorca agent-context`: the command schema agents read, from the service's command table and this CLI's own commands.

use clap::CommandFactory;
use massorca_protocol::capabilities::COMMANDS;
use massorca_protocol::codes;
use massorca_protocol::methods::commands::{CommandEntry, CommandTable};
use serde::Serialize;
use serde_json::Value;

use crate::args::Cli;
use crate::connect::connect;
use crate::error::{CliError, exit};
use crate::output::Output;
use crate::target::Target;

/// Version of the schema document; it changes only when a field is removed or changes meaning.
pub const SCHEMA_VERSION: u32 = 1;

/// Commands that run on this computer alone, as dotted paths; they make no service request. `service run` hosts the
/// service itself. The coverage test checks each path against the clap definition.
pub const LOCAL_COMMANDS: [&str; 10] = [
    "host.add-ssh",
    "host.discover",
    "host.list",
    "host.remove",
    "host.show",
    "host.status",
    "pair.accept",
    "profile.secure",
    "service.run",
    "service.unit",
];

/// The document `agent-context` prints: every command, sorted by its typed form.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentContext {
    pub schema_version: u32,
    pub command_count: usize,
    pub commands: Vec<AgentCommand>,
}

/// One command. A local command names no method, capability, permission or parameters.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentCommand {
    /// The command as typed, such as `git status`.
    pub command: String,
    pub summary: String,
    pub local: bool,
    /// The first request the command issues.
    pub method: Option<String>,
    pub capability: Option<String>,
    pub permission: Option<String>,
    /// JSON Schema of the parameters.
    pub args_schema: Option<Value>,
}

/// Joins the service's command table with this CLI's local commands, whose summaries come from the clap definition.
pub fn build(table: &CommandTable, root: &clap::Command) -> AgentContext {
    let mut commands: Vec<AgentCommand> = table.commands.iter().map(remote_command).collect();
    commands.extend(
        LOCAL_COMMANDS
            .iter()
            .filter_map(|path| local_command(root, path)),
    );
    commands.sort_by(|left, right| left.command.cmp(&right.command));
    AgentContext {
        schema_version: SCHEMA_VERSION,
        command_count: commands.len(),
        commands,
    }
}

fn remote_command(entry: &CommandEntry) -> AgentCommand {
    AgentCommand {
        command: entry.id.replace('.', " "),
        summary: entry.title.clone(),
        local: false,
        method: Some(entry.method.clone()),
        capability: entry.capability.clone(),
        permission: entry.permission.clone(),
        args_schema: Some(entry.args_schema.clone()),
    }
}

/// `None` only for a path the clap definition does not have, which the coverage test rules out.
fn local_command(root: &clap::Command, path: &str) -> Option<AgentCommand> {
    let leaf = path
        .split('.')
        .try_fold(root, |command, name| command.find_subcommand(name))?;
    Some(AgentCommand {
        command: path.replace('.', " "),
        summary: leaf
            .get_about()
            .map(ToString::to_string)
            .unwrap_or_default(),
        local: true,
        method: None,
        capability: None,
        permission: None,
        args_schema: None,
    })
}

/// The human listing: a count, then one line per command. Methods and parameters appear only in `--json`.
pub fn text_lines(schema: &AgentContext) -> Vec<String> {
    let width = schema
        .commands
        .iter()
        .map(|command| command.command.len())
        .max()
        .unwrap_or(0);
    let mut lines = vec![format!(
        "{} commands; --json adds each method, capability, permission and parameter list.",
        schema.command_count
    )];
    lines.extend(schema.commands.iter().map(|command| {
        let local = if command.local { " (local)" } else { "" };
        format!("{:<width$}  {}{local}", command.command, command.summary)
    }));
    lines
}

/// Refuses a service whose welcome does not offer `commands/1`: it predates the command table.
pub fn require_capability(offered: &[String]) -> Result<(), CliError> {
    if offered.iter().any(|name| name == COMMANDS) {
        return Ok(());
    }
    Err(CliError::new(
        codes::SERVICE_CAPABILITY_MISSING,
        format!(
            "the service does not offer {COMMANDS}, so it predates agent-context; restart it so it runs this version of MassOrca"
        ),
        exit::ERROR,
    ))
}

/// Prints the command schema of the service at `target` joined with this CLI's own commands.
pub async fn execute(target: &Target, autostart: bool, output: Output) -> Result<(), CliError> {
    let client = connect(target, autostart).await?;
    require_capability(&client.welcome().capabilities)?;
    let table = client.commands_describe().await?;
    let schema = build(&table, &Cli::command());
    output.success(&schema, &text_lines(&schema));
    Ok(())
}
