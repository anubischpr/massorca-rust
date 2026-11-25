//! The `massorca` command line: global options and the command tree.

use std::path::PathBuf;

use clap::builder::FalseyValueParser;
use clap::{ArgAction, Args, Parser, Subcommand};

use crate::agent::AgentCommand;
use crate::extension::ExtensionCommand;
use crate::marketplace::MarketplaceCommand;
use crate::skills::SkillCommand;
use crate::task::TaskCommand;
use crate::team::TeamCommand;
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
    /// Whether this run prints JSON.
    pub fn wants_json(&self) -> bool {
        self.json
    }
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Runs the service and controls the running one.
    Service {
        #[command(subcommand)]
        command: ServiceCommand,
    },
    /// Starts the MassOrca desktop app on this profile and returns at once.
    Open,
    /// Reads, changes and resets settings. `config` is an older name for it.
    #[command(alias = "config")]
    Settings {
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
    /// Checks and fixes who can reach the profile's folders on this computer.
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    /// Puts `massorca` on this user's PATH, and takes it off again. Works on this computer only.
    Cli {
        #[command(subcommand)]
        command: CliCommand,
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
    /// Lists, checks, removes and discovers the hosts this client paired with, and installs new ones over SSH.
    Host {
        #[command(subcommand)]
        command: HostCommand,
    },
    /// Lists every command with its method, permission and parameters, for agents.
    AgentContext,
    /// Runs coding agents in workspaces and lists the agent kinds.
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
    /// Reads and changes the git state of a workspace.
    Git {
        #[command(subcommand)]
        command: crate::git::GitCommand,
    },
    /// Stores secrets for agents and lists them by name. Values are never printed.
    Secret {
        #[command(subcommand)]
        command: crate::secret::SecretCommand,
    },
    /// Creates, lists, shows and removes Bot Teams: teams, their bots and projects, and their Boards.
    Team {
        #[command(subcommand)]
        command: TeamCommand,
    },
    /// Creates, lists, shows, finishes and removes the tasks on a team's Board.
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    /// Installs, lists, enables, disables and removes extensions, such as the agent packages.
    #[command(visible_alias = "ext")]
    Extension {
        #[command(subcommand)]
        command: ExtensionCommand,
    },
    /// Lists the skills agent tools and registered projects keep on this host, and the roots they were read from.
    Skills {
        #[command(subcommand)]
        command: SkillCommand,
    },
    /// Lists, connects and disconnects the hosted services, such as GitLab and Gitea, on a host.
    Marketplace {
        #[command(subcommand)]
        command: MarketplaceCommand,
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
    /// Shows the git, forge CLI and shell tools on the service's host. Works with --host.
    Tools {
        /// Detects the tools again instead of answering from the service's last detection.
        #[arg(long)]
        refresh: bool,
    },
    /// Prints the systemd user unit that runs the service on a Linux server.
    Unit(UnitArgs),
    /// The local trace logs of the profile.
    Log {
        #[command(subcommand)]
        command: ServiceLogCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ServiceLogCommand {
    /// Prints the logs folder and the newest service and desktop log files. Never starts the service.
    Path,
}

#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// Puts `massorca` on your PATH for this user only, without administrator rights: a link in `~/.local/bin` on
    /// Unix, a command file in a folder added to your Windows user PATH. Running it again changes nothing.
    Install,
    /// Removes what `cli install` made: the link or command file, and its PATH entry once its folder is empty.
    /// A file massorca did not make is left in place.
    Uninstall,
    /// Shows whether `massorca` is on your PATH and which binary it runs. Changes nothing.
    Status,
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    /// Makes the home private when other programs on this computer could change its files. Prints the access
    /// entries it removes and keeps, and changes them only with --yes; without it, it exits 0 having changed nothing.
    Secure(ProfileSecureArgs),
}

#[derive(Debug, Args)]
pub struct ProfileSecureArgs {
    /// Apply the change. Existing files are kept and trusted as they are, so review them first.
    #[arg(long)]
    pub yes: bool,
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
    /// Where an autostarting client reads why the service did not start; set by the client, not by people.
    #[arg(long, hide = true, value_name = "FILE")]
    pub start_status: Option<PathBuf>,
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
    /// Removes the stored value, so the setting's default applies again.
    Reset(ConfigGetArgs),
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
    /// Shows what `project add` without a kind would register for a directory, without registering it.
    Inspect {
        /// Directory to inspect. A relative path is taken from the current directory.
        path: PathBuf,
    },
    /// Unregisters a project. Refused while it has workspaces, unless their records are forgotten. Its files stay.
    Rm {
        /// Project id, name or path.
        project: String,
        /// Also forgets the project's workspace records. No file, folder, worktree or branch is touched.
        #[arg(long)]
        forget_workspaces: bool,
    },
    /// Sets the base that a project's new workspaces start from, or clears it with --clear.
    SetBase {
        /// Project id, name or path.
        project: String,
        /// A branch, remote branch or commit that resolves to a commit, such as `origin/develop`.
        #[arg(required_unless_present = "clear")]
        base: Option<String>,
        /// Clears the base, so new workspaces start from the repository's default branch.
        #[arg(long, conflicts_with = "base")]
        clear: bool,
    },
    /// Changes a project's icon: `--icon` sets it, `--clear-icon` removes it so the project shows the default one.
    Update {
        /// Project id, name or path.
        project: String,
        /// The icon: `emoji:<emoji>`, `lucide:<name>` or `image:<PNG file>`.
        #[arg(
            long,
            value_name = "ICON",
            conflicts_with = "clear_icon",
            required_unless_present = "clear_icon"
        )]
        icon: Option<String>,
        /// Removes the icon, so the project shows the default one.
        #[arg(long)]
        clear_icon: bool,
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
    /// Directory that holds this project's workspaces. Defaults to a folder named after the project under the workspaces default root, or to the default root itself when workspaces.layout is flat.
    #[arg(long, value_name = "DIR")]
    pub workspaces_root: Option<PathBuf>,
    /// The icon the project shows: `emoji:<emoji>`, `lucide:<name>` or `image:<PNG file>`. Unset, the icon the repository's files hold is used, if any.
    #[arg(long, value_name = "ICON")]
    pub icon: Option<String>,
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
    /// Shows the workspace that contains the current directory, or the one that contains --path.
    Current(WorkspaceCurrentArgs),
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
    /// Renames a workspace or sets its comment. A worktree's directory and branch keep their names.
    Update {
        /// Workspace id, name or path.
        workspace: String,
        /// The new name, checked the way a name given to `workspace create` is.
        #[arg(long)]
        name: Option<String>,
        /// The comment shown on the workspace; an empty one clears it.
        #[arg(long)]
        comment: Option<String>,
    },
    /// Checks that a workspace's folder exists and is readable and, for a worktree, that git still lists it.
    Check {
        /// Workspace id, name or path.
        workspace: String,
    },
}

#[derive(Debug, Args)]
pub struct WorkspaceCreateArgs {
    /// Project id, name or path.
    pub project: String,
    /// Name of the workspace, cleaned for its folder and branch (accents folded, other characters
    /// become `-`). Omit it to generate one.
    pub name: Option<String>,
    /// Branch to create or check out. Defaults to the workspace name, after the branch prefix
    /// (`workspaces.branch_prefix`) when one is set.
    #[arg(long, value_name = "BRANCH")]
    pub branch: Option<String>,
    /// Commit or ref the new branch starts from. `origin/x` means the remote-tracking branch. Defaults to the
    /// default branch (what `origin/HEAD` points at, else `origin/main`, `origin/master`, `main`, `master`), else HEAD.
    #[arg(long, value_name = "REF")]
    pub base: Option<String>,
    /// Check out an existing local branch instead of creating one.
    #[arg(long)]
    pub existing_branch: bool,
    /// Fetch the base from its remote first; a failed fetch starts from the local copy.
    #[arg(long)]
    pub refresh_base: bool,
    /// Directory of the worktree. A relative path is taken from the current directory.
    #[arg(long, value_name = "DIR")]
    pub path: Option<PathBuf>,
    /// Client request id: repeating it returns the first workspace instead of creating another.
    #[arg(long, value_name = "ID")]
    pub request_id: Option<String>,
    /// Do not run the setup script of the new worktree's `orca.yaml`.
    #[arg(long)]
    pub no_setup: bool,
}

#[derive(Debug, Args)]
pub struct WorkspaceListArgs {
    /// Only the workspaces of this project (id, name or path).
    #[arg(long, value_name = "PROJECT")]
    pub project: Option<String>,
}

#[derive(Debug, Args)]
pub struct WorkspaceCurrentArgs {
    /// Directory to look up. Defaults to the current directory; required with --host.
    #[arg(long, value_name = "DIR")]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ConfigGetArgs {
    /// Setting key, for example workspaces.default_root or workspaces.layout.
    pub key: String,
    /// Owner of the setting: core (default) or ext:<id>.
    #[arg(long, value_name = "OWNER")]
    pub owner: Option<String>,
}

#[derive(Debug, Args)]
pub struct ConfigSetArgs {
    /// Setting key, for example workspaces.default_root or workspaces.layout.
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
    /// Accepted for compatibility: a new device gets full access by default.
    #[arg(long, conflicts_with = "standard")]
    pub admin: bool,
    /// Advanced: a reduced device that can read and work in workspaces but not manage hosts or devices.
    #[arg(long)]
    pub standard: bool,
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
    /// Removes a revoked device from the list. Needs host-admin; an active device is refused.
    Forget {
        /// Device id, or its name when exactly one revoked device has it.
        device: String,
    },
    /// Gives a paired device full access, every scope, from its next connection. Needs host-admin.
    Grant {
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

#[derive(Debug, Subcommand)]
pub enum HostCommand {
    /// Lists the hosts this client paired with. Makes no connection.
    List,
    /// Shows one host: addresses, pin, device and scopes.
    Show {
        /// Host name.
        name: String,
    },
    /// Forgets a host. Unless --keep-device, first revokes this device on the server when it answers.
    Remove(HostRemoveArgs),
    /// Connects to one host, or to every host, and reports whether it answers.
    Status {
        /// Host name. Without it, every host is checked.
        name: Option<String>,
    },
    /// Lists the online Tailscale peers, and which of them run massorca.
    Discover,
    /// Installs or upgrades massorca on a Linux host over SSH, then pairs with it.
    AddSsh(HostAddSshArgs),
    /// Shows the memory of the service and of its terminals, read on the host that runs them. Works with --host.
    Memory,
    /// Shows the TCP ports that the processes of each live terminal listen on, read on the host that runs them. Works
    /// with --host.
    Ports,
}

#[derive(Debug, Args)]
pub struct HostRemoveArgs {
    /// Host name.
    pub name: String,
    /// Forget the host without revoking this device on the server.
    #[arg(long)]
    pub keep_device: bool,
    /// Also stops and removes the service over SSH. Needs --ssh.
    #[arg(long, requires = "ssh")]
    pub uninstall: bool,
    /// SSH destination of the host to uninstall, as ssh takes it: [user@]host.
    #[arg(long, value_name = "TARGET", requires = "uninstall")]
    pub ssh: Option<String>,
}

#[derive(Debug, Args)]
pub struct HostAddSshArgs {
    /// SSH destination, as ssh takes it: [user@]host.
    pub target: String,
    /// Linux massorca binary to install: an ELF file for x86_64 or aarch64.
    #[arg(long, value_name = "PATH")]
    pub binary: PathBuf,
    /// Host name to store the server as. Defaults to the server's name in the pairing offer.
    #[arg(long, value_name = "HOST")]
    pub name: Option<String>,
    /// Listen address for the service, repeatable. Defaults to tailscale.
    #[arg(long = "listen", value_name = "ADDR")]
    pub listen: Vec<String>,
    /// Extra option for ssh, repeatable: --ssh-arg -p --ssh-arg 2222.
    #[arg(long = "ssh-arg", value_name = "ARG", allow_hyphen_values = true)]
    pub ssh_arg: Vec<String>,
    /// Name this computer has on the server. Defaults to this computer's host name.
    #[arg(long, value_name = "NAME")]
    pub device_name: Option<String>,
}
