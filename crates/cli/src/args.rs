//! The `massorca` command line: global options and the command tree.

use std::ffi::OsString;
use std::path::PathBuf;

use clap::builder::FalseyValueParser;
use clap::{ArgAction, Parser, Subcommand};

/// Runs and controls the MassOrca service and reads and changes its settings.
#[derive(Debug, Parser)]
#[command(name = "massorca", version, arg_required_else_help = true)]
pub struct Cli {
    /// Profile to use. Each profile has its own service and data.
    #[arg(long, global = true, value_name = "NAME", env = "MASSORCA_PROFILE")]
    pub profile: Option<String>,

    /// Directory that holds the profiles. Defaults to the platform data directory.
    #[arg(long, global = true, value_name = "DIR", env = "MASSORCA_HOME")]
    pub home: Option<PathBuf>,

    /// Print one JSON document on stdout instead of text.
    #[arg(long, global = true)]
    pub json: bool,

    /// Never start the service on its own; fail when it is not running.
    #[arg(
        long,
        global = true,
        env = "MASSORCA_NO_AUTOSTART",
        action = ArgAction::SetTrue,
        value_parser = FalseyValueParser::new()
    )]
    pub no_autostart: bool,

    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    /// Whether this run prints JSON. `ext` takes its arguments verbatim, so a `--json` among them counts too.
    pub fn wants_json(&self) -> bool {
        self.json
            || matches!(&self.command, Commands::Ext { args } if args.iter().any(|arg| arg == "--json"))
    }
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Extensions. Not available yet: every form exits 1.
    Ext {
        /// Anything after `ext`; not interpreted yet.
        #[arg(allow_hyphen_values = true, num_args = 0.., value_name = "ARGS")]
        args: Vec<OsString>,
    },
}
