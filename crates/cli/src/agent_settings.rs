//! `massorca agent settings ...`: a kind's standing launch settings (decision 0174), the program, arguments and
//! variables every run of the kind starts with.
//!
//! Variable values are write-only: the service answers with names, and only names are printed. A secret value is read
//! from stdin or a prompt that does not echo, never from an argument.

use std::collections::BTreeMap;
use std::path::Path;

use clap::{Args, Subcommand};
use massorca_client::Client;
use massorca_protocol::methods::agents::{
    KindSettings, KindSettingsCleared, KindSettingsSetParams,
};

use crate::cwd_path;
use crate::error::{CliError, exit};
use crate::output::Output;
use crate::secret_input;

#[derive(Debug, Subcommand)]
pub enum SettingsCommand {
    /// Shows the kind's standing program, arguments and the names of its variables, never their values.
    Show {
        /// Agent kind, for example claude.
        kind: String,
    },
    /// Changes the kind's standing program, arguments or variables, which every terminal and chat run of the kind starts
    /// with, resumes included. A start's own `--env` and arguments win over them.
    Set(SettingsSetArgs),
    /// Removes the kind's standing settings; its runs start as the kind declares them again.
    Clear {
        /// Agent kind, for example claude.
        kind: String,
    },
}

#[derive(Debug, Args)]
pub struct SettingsSetArgs {
    /// Agent kind, for example claude.
    pub kind: String,
    /// The program that replaces the kind's own wherever the service runs it: an absolute path, a path made absolute
    /// against the current directory, or a bare name found on the service's search path. It must resolve now.
    #[arg(long, value_name = "PATH", conflicts_with = "no_program")]
    pub program: Option<String>,
    /// Remove the standing program: the kind's own runs again.
    #[arg(long)]
    pub no_program: bool,
    /// A standing argument, repeatable; any `--arg` replaces the standing arguments. They come after the kind's own and
    /// before a start's.
    #[arg(
        long = "arg",
        value_name = "ARG",
        allow_hyphen_values = true,
        conflicts_with = "no_args"
    )]
    pub args: Vec<String>,
    /// Remove the standing arguments.
    #[arg(long)]
    pub no_args: bool,
    /// Set a standing variable, repeatable. The value is visible in process listings while this command runs; use
    /// `--secret-env` for a secret.
    #[arg(long = "env", value_name = "KEY=VALUE", value_parser = parse_env)]
    pub env: Vec<(String, String)>,
    /// Set a standing variable whose value comes from stdin, or a prompt that does not echo.
    #[arg(long = "secret-env", value_name = "KEY")]
    pub secret_env: Option<String>,
    /// Remove a standing variable, repeatable.
    #[arg(long = "unset-env", value_name = "KEY")]
    pub unset_env: Vec<String>,
}

/// `KEY=VALUE`, split at the first `=`; the key must not be empty.
///
/// # Errors
/// The text has no `=`, or nothing before it.
pub fn parse_env(text: &str) -> Result<(String, String), String> {
    match text.split_once('=') {
        Some((key, value)) if !key.is_empty() => Ok((key.to_owned(), value.to_owned())),
        _ => Err("expected KEY=VALUE".to_owned()),
    }
}

/// The variables of repeated `--env KEY=VALUE` options; a later one of the same key wins.
#[must_use]
pub fn env_map(pairs: Vec<(String, String)>) -> BTreeMap<String, String> {
    pairs.into_iter().collect()
}

/// Runs one `agent settings` subcommand.
pub async fn execute(
    client: &Client,
    command: SettingsCommand,
    output: Output,
) -> Result<(), CliError> {
    match command {
        SettingsCommand::Show { kind } => {
            let settings = client.agent_settings_get(&kind).await?;
            output.success(&settings, &settings_lines(&settings));
        }
        SettingsCommand::Set(args) => {
            let params = set_params(args)?;
            let settings = client.agent_settings_set(&params).await?;
            output.success(&settings, &settings_lines(&settings));
        }
        SettingsCommand::Clear { kind } => {
            let cleared = client.agent_settings_clear(&kind).await?;
            output.success(&cleared, &[cleared_line(&cleared)]);
        }
    }
    Ok(())
}

fn set_params(args: SettingsSetArgs) -> Result<KindSettingsSetParams, CliError> {
    let program = match (args.program, args.no_program) {
        (_, true) => Some(String::new()),
        (Some(program), false) if program.contains(['/', '\\']) => {
            Some(cwd_path::absolute(Path::new(&program))?)
        }
        (program, false) => program,
    };
    let args_change = if args.no_args {
        Some(Vec::new())
    } else if args.args.is_empty() {
        None
    } else {
        Some(args.args)
    };
    let mut env = env_map(args.env);
    if let Some(name) = args.secret_env {
        let value = secret_input::read_hidden_or_stdin(&format!("Value for {name}: "))?;
        env.insert(name, value);
    }
    let params = KindSettingsSetParams {
        kind: args.kind,
        program,
        args: args_change,
        env,
        remove_env: args.unset_env,
    };
    if params.program.is_none()
        && params.args.is_none()
        && params.env.is_empty()
        && params.remove_env.is_empty()
    {
        return Err(CliError::new(
            "request.invalid_params",
            "name something to change: --program, --no-program, --arg, --no-args, --env, --secret-env or --unset-env",
            exit::USAGE,
        ));
    }
    Ok(params)
}

fn settings_lines(settings: &KindSettings) -> Vec<String> {
    let program = settings
        .program
        .clone()
        .unwrap_or_else(|| "the kind's own".to_owned());
    let arguments = if settings.args.is_empty() {
        "none".to_owned()
    } else {
        settings.args.join(" ")
    };
    let environment = if settings.env_names.is_empty() {
        "none".to_owned()
    } else {
        settings.env_names.join(", ")
    };
    vec![
        format!("launch settings of {}", settings.kind),
        format!("program: {program}"),
        format!("arguments: {arguments}"),
        format!("environment: {environment}"),
    ]
}

fn cleared_line(cleared: &KindSettingsCleared) -> String {
    if cleared.removed {
        format!("removed the launch settings of {}", cleared.kind)
    } else {
        format!("{} had no launch settings", cleared.kind)
    }
}

#[cfg(test)]
mod tests {
    use super::parse_env;

    #[test]
    fn an_env_option_splits_at_the_first_equals_sign() {
        assert_eq!(
            parse_env("URL=a=b"),
            Ok(("URL".to_owned(), "a=b".to_owned()))
        );
        assert_eq!(parse_env("EMPTY="), Ok(("EMPTY".to_owned(), String::new())));
        assert!(parse_env("NOVALUE").is_err());
        assert!(parse_env("=value").is_err());
    }
}
