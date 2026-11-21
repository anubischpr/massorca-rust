//! The `massorca` command line: global options and the command tree.

use std::ffi::OsString;
use std::path::PathBuf;

use clap::builder::FalseyValueParser;
use clap::{ArgAction, Args, Parser, Subcommand};

use crate::terminal::TerminalCommand;

/// Runs and controls the MassOrca service and reads and changes its settings.
#[derive(Debug, Parser)]
#[command(name = "massorca", version, arg_required_else_help = true)]
pub struct Cli {
    /// Profile to use. Each profile has its own service and data.
    #[arg(long, global = true, value_name = "NAME", env = "MASSORCA_PROFILE")]
    pub profile: Option<String>,

    // No env attribute: `Profile` reads MASSORCA_HOME itself, so `service unit` never takes it.
    /// Directory that holds the profiles. Defaults to the platform data directory.
    #[arg(long, global = true, value_name = "DIR")]
    pub home: Option<PathBuf>,

    /// Host to address: a paired host name, or local (the default).
    #[arg(long, global = true, value_name = "NAME", env = "MASSORCA_HOST")]
    pub host: Option<String>,

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
    /// Runs the service and controls the running one.
    Service {
        #[command(subcommand)]
        command: ServiceCommand,
    },
    /// Reads and changes settings.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Registers, lists, shows and removes projects: git repositories and folders.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Creates, lists, shows and removes workspaces: worktrees and folders of a project.
    #[command(visible_alias = "ws")]
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    /// Starts terminals in workspaces and works with them: type, read, attach, resize and stop.
    Terminal {
        #[command(subcommand)]
        command: TerminalCommand,
    },
    /// Makes pairing offers for other computers, and pairs this computer with a server.
    Pair {
        #[command(subcommand)]
        command: PairCommand,
    },
    /// Lists, revokes and identifies the devices paired with the service.
    Device {
        #[command(subcommand)]
        command: DeviceCommand,
    },
    /// Shows the network state of the service, and this computer's Tailscale state.
    Server {
        #[command(subcommand)]
        command: ServerCommand,
    },
    /// Extensions. Not available yet: every form exits 1.
    Ext {
        /// Anything after `ext`; not interpreted yet.
        #[arg(allow_hyphen_values = true, num_args = 0.., value_name = "ARGS")]
        args: Vec<OsString>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ServiceCommand {
    /// Runs the service in this process until it is stopped.
    Run(RunArgs),
    /// Shows whether the service is running. Never starts it.
    Status,
    /// Shows the service's identity. Starts the service when it is not running.
    Info,
    /// Stops the service. Succeeds when it is not running.
    Stop,
    /// Stops the service, then starts it again.
    Restart,
    /// Prints the systemd user unit that runs the service on a Linux server.
    Unit(UnitArgs),
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Also write log lines to stderr.
    #[arg(long)]
    pub log_stderr: bool,
    /// Address for network clients, repeatable: tailscale[:PORT], localhost:PORT or IP:PORT. Also MASSORCA_LISTEN.
    #[arg(
        long = "listen",
        value_name = "ADDR",
        env = "MASSORCA_LISTEN",
        value_delimiter = ','
    )]
    pub listen: Vec<String>,
    /// Allows a listen address on every interface (0.0.0.0 or ::), which public networks can reach.
    #[arg(long)]
    pub allow_all_interfaces: bool,
}

#[derive(Debug, Args)]
pub struct UnitArgs {
    /// Listen address the unit passes to `service run`, repeatable. Defaults to tailscale.
    #[arg(long = "listen", value_name = "ADDR")]
    pub listen: Vec<String>,
    /// Binary the unit runs. Defaults to %h/.massorca/bin/massorca.
    #[arg(long, value_name = "PATH")]
    pub exe: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Shows one setting and whether it still has its default value.
    Get(ConfigGetArgs),
    /// Changes one setting. The value is parsed as JSON when it is valid JSON, otherwise taken as text.
    Set(ConfigSetArgs),
    /// Lists every setting.
    List,
}

#[derive(Debug, Subcommand)]
pub enum ProjectCommand {
    /// Registers a git repository, or a folder with --folder. Without a kind, a directory inside a repository is a git project.
    Add(ProjectAddArgs),
    /// Lists every project, ordered by name.
    List,
    /// Shows one project, selected by id, name or path.
    Show {
        /// Project id, name or path.
        project: String,
    },
    /// Unregisters a project that has no workspaces. Its files stay.
    Rm {
        /// Project id, name or path.
        project: String,
    },
}

#[derive(Debug, Args)]
pub struct ProjectAddArgs {
    /// Directory to register. A relative path is taken from the current directory.
    pub path: PathBuf,
    /// Name of the project. Defaults to the directory's name.
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    /// Register the directory as a folder, even when it is inside a repository.
    #[arg(long, conflicts_with = "git")]
    pub folder: bool,
    /// Register a git repository; fail when the directory is not inside one.
    #[arg(long)]
    pub git: bool,
    /// Directory that holds this project's workspaces. Defaults to a folder under the workspaces default root.
    #[arg(long, value_name = "DIR")]
    pub workspaces_root: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum WorkspaceCommand {
    /// Creates a worktree on a new branch (git project), or registers a folder (folder project).
    Create(WorkspaceCreateArgs),
    /// Lists every workspace, or only those of one project.
    List(WorkspaceListArgs),
    /// Shows one workspace, selected by id, name or path.
    Show {
        /// Workspace id, name or path.
        workspace: String,
    },
    /// Removes a workspace. A worktree is deleted with its directory; a folder is only forgotten.
    Rm {
        /// Workspace id, name or path.
        workspace: String,
        /// Discard uncommitted and untracked changes in the worktree.
        #[arg(long)]
        force: bool,
        /// Delete ignored files in the worktree. Git does not list them, and force does not cover them.
        #[arg(long)]
        discard_ignored: bool,
        /// Forget a workspace whose directory git does not recognise, keeping the directory.
        #[arg(long)]
        forget_orphan: bool,
    },
}

#[derive(Debug, Args)]
pub struct WorkspaceCreateArgs {
    /// Project id, name or path.
    pub project: String,
    /// Name of the workspace; also its branch and directory name unless set otherwise.
    pub name: String,
    /// Branch to create or check out. Defaults to the workspace name.
    #[arg(long, value_name = "BRANCH")]
    pub branch: Option<String>,
    /// Commit or ref the new branch starts from. Defaults to HEAD.
    #[arg(long, value_name = "REF")]
    pub base: Option<String>,
    /// Check out an existing local branch instead of creating one.
    #[arg(long)]
    pub existing_branch: bool,
    /// Directory of the worktree. A relative path is taken from the current directory.
    #[arg(long, value_name = "DIR")]
    pub path: Option<PathBuf>,
    /// Client request id: repeating it returns the first workspace instead of creating another.
    #[arg(long, value_name = "ID")]
    pub request_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct WorkspaceListArgs {
    /// Only the workspaces of this project (id, name or path).
    #[arg(long, value_name = "PROJECT")]
    pub project: Option<String>,
}

#[derive(Debug, Args)]
pub struct ConfigGetArgs {
    /// Setting key, for example workspaces.default_root.
    pub key: String,
    /// Owner of the setting: core (default) or ext:<id>.
    #[arg(long, value_name = "OWNER")]
    pub owner: Option<String>,
}

#[derive(Debug, Args)]
pub struct ConfigSetArgs {
    /// Setting key, for example workspaces.default_root.
    pub key: String,
    /// New value: JSON when it parses, otherwise plain text.
    #[arg(allow_hyphen_values = true)]
    pub value: String,
    /// Owner of the setting: core (default) or ext:<id>.
    #[arg(long, value_name = "OWNER")]
    pub owner: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum PairCommand {
    /// Makes a pairing offer on the service. Only its owner or a device-admin can, and it works once.
    Offer(PairOfferArgs),
    /// Pairs this computer with the server of an offer, and stores the server as a host.
    Accept(PairAcceptArgs),
}

#[derive(Debug, Args)]
pub struct PairOfferArgs {
    /// The new device may also manage hosts and devices (host-admin and device-admin).
    #[arg(long)]
    pub admin: bool,
    /// Seconds the offer stays valid: 1 to 3600. The service default is 600.
    #[arg(long, value_name = "SECS")]
    pub ttl: Option<u32>,
}

#[derive(Debug, Args)]
pub struct PairAcceptArgs {
    /// The offer string, or - to read it from stdin, which keeps it out of shell history.
    pub offer: String,
    /// Host name to store the server as. Defaults to the server's name in the offer.
    #[arg(long, value_name = "HOST")]
    pub name: Option<String>,
    /// Name this computer has on the server. Defaults to this computer's host name.
    #[arg(long, value_name = "NAME")]
    pub device_name: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum DeviceCommand {
    /// Lists the paired devices, their scopes and state. Needs device-admin.
    List,
    /// Revokes a device: its connections close at once and it is refused afterwards.
    Revoke {
        /// Device id, or its name when that is unique.
        device: String,
    },
    /// Shows the device this command runs as.
    Whoami,
}

#[derive(Debug, Subcommand)]
pub enum ServerCommand {
    /// Shows the listeners, server pin, devices and pending offers; on this computer also Tailscale.
    Status,
}
