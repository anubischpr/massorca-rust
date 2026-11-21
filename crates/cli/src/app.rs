//! Runs one command line: parses it, dispatches to the command, and turns the outcome into output and an exit code.

use std::ffi::OsString;
use std::process::ExitCode;

use clap::Parser;

use crate::args::{Cli, Commands};
use crate::error::CliError;
use crate::ext;
use crate::output::{self, Output};

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
    match dispatch(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            output.failure(&error);
            ExitCode::from(error.exit_code)
        }
    }
}

fn dispatch(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Commands::Ext { .. } => Err(ext::unavailable()),
    }
}
