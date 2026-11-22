//! Runs one command line: parses it, dispatches to the command, and turns the outcome into output and an exit code.

use std::ffi::OsString;
use std::future::Future;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use massorca_profile::Profile;
use tokio::runtime::Builder;

use crate::agent;
use crate::agent_hook;
use crate::args::{Cli, Commands, PairCommand, ProfileCommand, ServerCommand, ServiceCommand};
use crate::config;
use crate::device;
use crate::error::CliError;
use crate::ext;
use crate::host;
use crate::output::{self, Output};
use crate::pair;
use crate::profile_secure;
use crate::project;
use crate::server_status;
use crate::service::{control, foreground, unit};
use crate::target::{Target, reject_host};
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

/// The global options of one run, as the commands need them.
struct Globals {
    profile: Option<String>,
    home: Option<PathBuf>,
    host: Option<String>,
    autostart: bool,
}

impl Globals {
    /// The profile this run uses: `--profile`, then `MASSORCA_PROFILE`, then the default.
    fn profile(&self) -> Result<Profile, CliError> {
        Ok(Profile::resolve(self.home.clone(), self.profile.clone())?)
    }

    /// The service this run addresses: the local service of the profile, or the `--host` one.
    fn target(&self) -> Result<Target, CliError> {
        Ok(Target::new(self.profile()?, self.host.clone()))
    }

    /// Refuses `--host` for a command that works on this computer only.
    fn local_only(&self, command: &str) -> Result<(), CliError> {
        reject_host(self.host.as_deref(), command)
    }
}

fn dispatch(cli: Cli, output: Output) -> Result<(), CliError> {
    let Cli {
        profile,
        home,
        host,
        no_autostart,
        json,
        command,
    } = cli;
    let globals = Globals {
        profile,
        home,
        host,
        autostart: !no_autostart,
    };
    let autostart = globals.autostart;
    match command {
        Commands::Service { command } => service_command(&globals, command, output),
        Commands::Settings { command } => {
            let target = globals.target()?;
            block_on(config::execute(&target, autostart, command, output))
        }
        Commands::Project { command } => {
            let target = globals.target()?;
            block_on(project::execute(&target, autostart, command, output))
        }
        Commands::Workspace { command } => {
            let target = globals.target()?;
            block_on(workspace::execute(&target, autostart, command, output))
        }
        Commands::Terminal { command } => {
            let target = globals.target()?;
            block_on(terminal::execute(&target, autostart, command, output, json))
        }
        Commands::Profile {
            command: ProfileCommand::Secure(args),
        } => {
            globals.local_only("profile secure")?;
            profile_secure::run(&globals.profile()?, &args, output)
        }
        Commands::Pair { command } => pair_command(&globals, command, output),
        Commands::Device { command } => {
            let target = globals.target()?;
            block_on(device::execute(&target, autostart, command, output))
        }
        Commands::Server { command } => {
            let target = globals.target()?;
            match command {
                ServerCommand::Status => {
                    block_on(server_status::execute(&target, autostart, output))
                }
            }
        }
        Commands::Host { command } => {
            globals.local_only(host::command_path(&command))?;
            block_on(host::execute(&globals.profile()?, command, output))
        }
        Commands::Agent {
            command: agent::AgentCommand::Hook(args),
        } => {
            // Fails open: never an error, never a service start (0053).
            // The hook runs on the host that runs the agent, so it always reports to the local service.
            agent_hook::run(
                globals.home.clone(),
                globals.profile.clone(),
                args.event,
                args.payload,
            );
            Ok(())
        }
        Commands::Agent { command } => {
            let target = globals.target()?;
            block_on(agent::execute(&target, autostart, command, output, json))
        }
        Commands::Git { command } => {
            let target = globals.target()?;
            block_on(crate::git::execute(&target, autostart, command, output))
        }
        Commands::Secret { command } => {
            let target = globals.target()?;
            block_on(crate::secret::execute(&target, autostart, command, output))
        }
        Commands::Team { command } => {
            let target = globals.target()?;
            block_on(crate::team::execute(&target, autostart, command, output))
        }
        Commands::Task { command } => {
            let target = globals.target()?;
            block_on(crate::task::execute(&target, autostart, command, output))
        }
        Commands::Ext { .. } => Err(ext::unavailable()),
    }
}

/// `service ...`: the service of the profile, run here or controlled; `unit` prints a unit.
fn service_command(
    globals: &Globals,
    command: ServiceCommand,
    output: Output,
) -> Result<(), CliError> {
    match command {
        ServiceCommand::Run(args) => {
            globals.local_only("service run")?;
            foreground::run(&globals.profile()?, &args, output)
        }
        ServiceCommand::Status => {
            globals.local_only("service status")?;
            block_on(control::status(&globals.profile()?, output))
        }
        ServiceCommand::Info => {
            let target = globals.target()?;
            block_on(control::info(&target, globals.autostart, output))
        }
        ServiceCommand::Stop => {
            globals.local_only("service stop")?;
            block_on(control::stop(&globals.profile()?, output))
        }
        ServiceCommand::Restart => {
            globals.local_only("service restart")?;
            block_on(control::restart(&globals.profile()?, output))
        }
        ServiceCommand::Unit(args) => {
            globals.local_only("service unit")?;
            unit::print(&args, globals.home.as_deref(), output)
        }
    }
}

/// `pair ...`: an offer for another computer, or pairing this one with a server.
fn pair_command(globals: &Globals, command: PairCommand, output: Output) -> Result<(), CliError> {
    match command {
        PairCommand::Offer(args) => {
            let target = globals.target()?;
            block_on(pair::offer(&target, globals.autostart, &args, output))
        }
        PairCommand::Accept(args) => {
            globals.local_only("pair accept")?;
            block_on(pair::accept(&globals.profile()?, &args, output))
        }
    }
}

/// Runs one async command on a current-thread runtime.
fn block_on<T>(future: impl Future<Output = Result<T, CliError>>) -> Result<T, CliError> {
    let runtime = Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(future)
}
