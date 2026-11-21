//! Runs one command line: parses it, dispatches to the command, and turns the outcome into output and an exit code.

use std::ffi::OsString;
use std::process::ExitCode;

use clap::Parser;
use massorca_profile::Profile;

use crate::args::{Cli, Commands, ServiceCommand};
use crate::error::CliError;
use crate::ext;
use crate::output::{self, Output};
use crate::service::foreground;

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
        command,
        ..
    } = cli;
    match command {
        Commands::Service { command } => {
            let profile = Profile::resolve(home, profile)?;
            match command {
                ServiceCommand::Run(args) => foreground::run(&profile, &args, output),
            }
        }
        Commands::Ext { .. } => Err(ext::unavailable()),
    }
}
