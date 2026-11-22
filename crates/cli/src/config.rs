//! `massorca settings get|set|list|reset` (also `massorca config`): reading and changing settings through the service.

use massorca_profile::Profile;
use massorca_protocol::methods::config::{ConfigKey, ConfigSet, ConfigValue};
use serde_json::Value;

use crate::args::{ConfigCommand, ConfigGetArgs, ConfigSetArgs};
use crate::connect::connect;
use crate::error::CliError;
use crate::output::Output;

/// Runs one `config` subcommand against the service of `profile`.
pub async fn execute(
    profile: &Profile,
    autostart: bool,
    command: ConfigCommand,
    output: Output,
) -> Result<(), CliError> {
    match command {
        ConfigCommand::Get(args) => get(profile, autostart, args, output).await,
        ConfigCommand::Set(args) => set(profile, autostart, args, output).await,
        ConfigCommand::List => list(profile, autostart, output).await,
        ConfigCommand::Reset(args) => reset(profile, autostart, args, output).await,
    }
}

async fn reset(
    profile: &Profile,
    autostart: bool,
    args: ConfigGetArgs,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(profile, autostart).await?;
    let value = client
        .config_reset(&ConfigKey {
            owner: args.owner,
            key: args.key,
        })
        .await?;
    output.success(
        &value,
        &[format!(
            "{} = {} (default)",
            value.key,
            render(&value.value)
        )],
    );
    Ok(())
}

async fn get(
    profile: &Profile,
    autostart: bool,
    args: ConfigGetArgs,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(profile, autostart).await?;
    let value = client
        .config_get(&ConfigKey {
            owner: args.owner,
            key: args.key,
        })
        .await?;
    output.success(&value, &[render(&value.value)]);
    Ok(())
}

async fn set(
    profile: &Profile,
    autostart: bool,
    args: ConfigSetArgs,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(profile, autostart).await?;
    let params = ConfigSet {
        owner: args.owner,
        key: args.key,
        value: parse_value(&args.value),
    };
    let value = client.config_set(&params).await?;
    output.success(
        &value,
        &[format!("{} = {}", value.key, render(&value.value))],
    );
    Ok(())
}

async fn list(profile: &Profile, autostart: bool, output: Output) -> Result<(), CliError> {
    let client = connect(profile, autostart).await?;
    let list = client.config_list().await?;
    let lines: Vec<String> = list.values.iter().map(list_line).collect();
    output.success(&list, &lines);
    Ok(())
}

fn list_line(value: &ConfigValue) -> String {
    let default = if value.is_default { " (default)" } else { "" };
    format!(
        "{} {} = {}{default}",
        value.owner,
        value.key,
        render(&value.value)
    )
}

/// The value as typed: JSON when the text parses as JSON, otherwise a plain string.
fn parse_value(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_owned()))
}

/// Strings print bare, so a path can be used in scripts; other values print as JSON.
fn render(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}
