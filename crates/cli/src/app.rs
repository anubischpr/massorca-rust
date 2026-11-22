//! Runs one command line: parses it, dispatches to the command, and turns the outcome into output and an exit code.

use std::ffi::OsString;
use std::future::Future;
use std::process::ExitCode;

use clap::Parser;
use massorca_profile::Profile;
use tokio::runtime::Builder;

use crate::agent;
use crate::args::{Cli, Commands, ServiceCommand};
use crate::config;
use crate::error::CliError;
use crate::ext;
use crate::output::{self, Output};
use crate::project;
use crate::service::{control, foreground};
use crate::terminal;
use crate::workspace;

/// Runs the command line `args` (program name first) and returns its exit code.
pub fn run<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let json_requested = args.iter().any(|arg| arg == "--json");
    let cli = match Cli::try_parse_from(args.iter().cloned()) {
        Ok(cli) => cli,
        Err(error) => return output::usage(error, json_requested),
    };
    let output = Output::new(cli.wants_json());
    match dispatch(cli, output) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            output.failure(&error);
            ExitCode::from(error.exit_code)
        }
    }
}

fn dispatch(cli: Cli, output: Output) -> Result<(), CliError> {
    let Cli {
        profile,
        home,
        no_autostart,
        json,
        command,
    } = cli;
    let autostart = !no_autostart;
    match command {
        Commands::Service { command } => {
            let profile = Profile::resolve(home, profile)?;
            match command {
                ServiceCommand::Run(args) => foreground::run(&profile, &args, output),
                ServiceCommand::Status => block_on(control::status(&profile, output)),
                ServiceCommand::Info => block_on(control::info(&profile, autostart, output)),
                ServiceCommand::Stop => block_on(control::stop(&profile, output)),
                ServiceCommand::Restart => block_on(control::restart(&profile, output)),
            }
        }
        Commands::Config { command } => {
            let profile = Profile::resolve(home, profile)?;
            block_on(config::execute(&profile, autostart, command, output))
        }
        Commands::Project { command } => {
            let profile = Profile::resolve(home, profile)?;
            block_on(project::execute(&profile, autostart, command, output))
        }
        Commands::Workspace { command } => {
            let profile = Profile::resolve(home, profile)?;
            block_on(workspace::execute(&profile, autostart, command, output))
        }
        Commands::Terminal { command } => {
            let profile = Profile::resolve(home, profile)?;
            block_on(terminal::execute(
                &profile, autostart, command, output, json,
            ))
        }
        Commands::Agent { command } => {
            let profile = Profile::resolve(home, profile)?;
            block_on(agent::execute(&profile, autostart, command, output, json))
        }
        Commands::Git { command } => {
            let profile = Profile::resolve(home, profile)?;
            block_on(crate::git::execute(&profile, autostart, command, output))
        }
        Commands::Secret { command } => {
            let profile = Profile::resolve(home, profile)?;
            block_on(crate::secret::execute(&profile, autostart, command, output))
        }
        Commands::Ext { .. } => Err(ext::unavailable()),
    }
}

/// Runs one async command on a current-thread runtime.
fn block_on<T>(future: impl Future<Output = Result<T, CliError>>) -> Result<T, CliError> {
    let runtime = Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(future)
}
