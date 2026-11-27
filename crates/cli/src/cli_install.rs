//! `massorca cli install|uninstall|status`: puts the `massorca` command on this user's PATH and nothing else (decision 0088).

pub mod command_file;
#[cfg(test)]
pub mod fake_user_path;
pub mod layout;
pub mod path_entries;
pub mod plan;
pub mod user_path;
#[cfg(windows)]
pub mod windows_user_path;

use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::args::CliCommand;
use crate::error::{CliError, exit};
use crate::output::Output;
use layout::{Layout, Platform, same_path};
use plan::{InstallStep, Observed, State, UninstallStep};
use user_path::{PathValue, UserPath};

/// Error codes of the command, so a script can match on them.
const CONFLICT: &str = "cli.command_conflict";
const INVALID_TARGET: &str = "cli.invalid_target";
const NO_HOME: &str = "cli.no_home_directory";

/// Where one run puts the command and which PATH it reads.
#[derive(Debug)]
pub enum Environment {
    /// Unix: a link in `~/.local/bin`, and the PATH this process inherited, which is only read.
    Unix {
        layout: Layout,
        target: PathBuf,
        path: String,
    },
    /// Windows: a command file in a per-user folder, and the user PATH behind `store`.
    Windows {
        layout: Layout,
        target: PathBuf,
        store: Box<dyn UserPath>,
        variables: Vec<(String, String)>,
    },
}

impl Environment {
    /// The environment of this process. Fails when the folder the command lives in cannot be found.
    pub fn from_process() -> Result<Self, CliError> {
        let target = current_binary()?;
        match Platform::current() {
            Platform::Unix => Ok(Self::Unix {
                layout: Layout::unix(&absolute_env_dir("HOME")?),
                target,
                path: std::env::var("PATH").unwrap_or_default(),
            }),
            Platform::Windows => Ok(Self::Windows {
                layout: Layout::windows(&absolute_env_dir("LOCALAPPDATA")?),
                target,
                store: system_user_path(),
                variables: std::env::vars_os()
                    .map(|(key, value)| {
                        (
                            key.to_string_lossy().into_owned(),
                            value.to_string_lossy().into_owned(),
                        )
                    })
                    .collect(),
            }),
        }
    }

    fn layout(&self) -> &Layout {
        match self {
            Self::Unix { layout, .. } | Self::Windows { layout, .. } => layout,
        }
    }

    fn target(&self) -> &Path {
        match self {
            Self::Unix { target, .. } | Self::Windows { target, .. } => target.as_path(),
        }
    }

    /// The user PATH, read now; `None` on Unix, where the PATH is only read from the process.
    fn user_path(&self) -> Result<Option<PathValue>, CliError> {
        match self {
            Self::Unix { .. } => Ok(None),
            Self::Windows { store, .. } => store.read().map(Some),
        }
    }

    /// Whether the command folder is on PATH; `None` when the PATH cannot be read.
    fn bin_dir_on_path(&self) -> Option<bool> {
        match self {
            Self::Unix { layout, path, .. } => Some(path_entries::contains(
                Platform::Unix,
                path,
                &layout.bin_dir,
                &[],
            )),
            Self::Windows {
                layout,
                store,
                variables,
                ..
            } => store.read().ok().map(|value| {
                path_entries::contains(Platform::Windows, &value.text, &layout.bin_dir, variables)
            }),
        }
    }
}

/// What install, uninstall and status report, as the JSON result and as text lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub state: State,
    /// `symlink` on Unix, `command_file` on Windows.
    pub method: &'static str,
    pub command: String,
    /// The binary this CLI runs; the command should run it too.
    pub target: String,
    /// The binary the command runs now, when it is massorca's.
    pub points_to: Option<String>,
    pub bin_dir: String,
    /// Whether the command folder is on PATH; `null` when the PATH could not be read.
    pub bin_dir_on_path: Option<bool>,
    /// Whether this run changed anything.
    pub changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Install,
    Uninstall,
    Status,
}

/// Points the command at this binary and makes sure its folder is on PATH. Running it again changes nothing.
pub fn install(environment: &Environment) -> Result<Report, CliError> {
    let location = environment.layout();
    let target = environment.target();
    if location.platform == Platform::Windows && layout::command_file_text(target).is_none() {
        return Err(invalid_target(target));
    }
    // Read before anything changes: a PATH that cannot be read stops the install.
    let user_path = environment.user_path()?;
    let observed = observe_command(location, target)?;
    let step = plan::install_step(&observed, target, location.platform);
    if step == InstallStep::Refuse {
        return Err(conflict(location, Operation::Install));
    }
    let mut changed = false;
    if step != InstallStep::Keep {
        command_file::place(location, target)?;
        changed = true;
    }
    if let (
        Some(value),
        Environment::Windows {
            store, variables, ..
        },
    ) = (user_path, environment)
    {
        changed |= add_bin_dir_to_user_path(&**store, variables, &value, location)?;
    }
    report(environment, changed)
}

/// Removes the command this install made, and its PATH entry once nothing else is in its folder.
pub fn uninstall(environment: &Environment) -> Result<Report, CliError> {
    let location = environment.layout();
    let observed = observe_command(location, environment.target())?;
    let mut changed = false;
    match plan::uninstall_step(&observed) {
        UninstallStep::Refuse => return Err(conflict(location, Operation::Uninstall)),
        UninstallStep::Remove => {
            command_file::remove(location)?;
            changed = true;
        }
        UninstallStep::Nothing => {}
    }
    if let Environment::Windows {
        store, variables, ..
    } = environment
    {
        changed |= clear_bin_dir_from_user_path(&**store, variables, location)?;
    }
    report(environment, changed)
}

/// Reports the command's state without changing anything.
pub fn status(environment: &Environment) -> Result<Report, CliError> {
    report(environment, false)
}

/// Points a command whose binary is gone at this binary; returns whether it changed the command. A command that runs
/// another binary that is still there is left alone, so a development build never takes over an install (decision 0113).
pub fn repair_gone(environment: &Environment) -> Result<bool, CliError> {
    let location = environment.layout();
    let target = environment.target();
    if !matches!(observe_command(location, target)?, Observed::Gone { .. }) {
        return Ok(false);
    }
    if location.platform == Platform::Windows && layout::command_file_text(target).is_none() {
        return Err(invalid_target(target));
    }
    command_file::place(location, target)?;
    Ok(true)
}

/// Runs when the desktop starts this service with `--repair-command`: repoints a stale command at this binary. Best
/// effort: a failure is logged and never stops the service.
pub fn repair_on_service_start() {
    match Environment::from_process().and_then(|environment| repair_gone(&environment)) {
        Ok(true) => tracing::info!("the massorca command was repointed at this binary"),
        Ok(false) => {}
        Err(error) => tracing::warn!(
            code = error.code,
            error = %error.message,
            "the massorca command could not be repointed"
        ),
    }
}

/// The text lines of one outcome; JSON output carries the report itself.
pub fn lines(operation: Operation, report: &Report, platform: Platform) -> Vec<String> {
    let command = &report.command;
    let folder = &report.bin_dir;
    let mut lines = match (operation, report.state) {
        (Operation::Uninstall, State::NotInstalled) if report.changed => {
            vec![format!("Removed {command}.")]
        }
        (Operation::Uninstall, _) => {
            vec![format!(
                "No massorca command at {command}; nothing to remove."
            )]
        }
        (Operation::Install, State::Installed) if report.changed => {
            vec![format!(
                "Installed massorca at {command}; it runs {}.",
                report.target
            )]
        }
        (_, State::Installed) => vec![format!("massorca runs {} from {command}.", report.target)],
        (_, State::NotOnPath) => vec![
            format!("massorca is installed at {command}, but {folder} is not on your PATH."),
            off_path_hint(platform),
        ],
        (_, State::DifferentBinary) => vec![
            format!(
                "{command} runs {}, not {}.",
                report.points_to.as_deref().unwrap_or("another program"),
                report.target
            ),
            "Run `massorca cli install` to point it at this binary.".to_owned(),
        ],
        (_, State::Stale) => vec![
            format!(
                "{command} runs {}, which is no longer there.",
                report.points_to.as_deref().unwrap_or("another program")
            ),
            "Starting the app points it at this binary; `massorca cli install` does it now."
                .to_owned(),
        ],
        (_, State::NotInstalled) => vec![
            "massorca is not installed. Run `massorca cli install` to put it on your PATH."
                .to_owned(),
        ],
        (_, State::Conflict) => vec![format!(
            "{command} was not made by `massorca cli install`, so it was left alone."
        )],
    };
    if report.bin_dir_on_path.is_none() {
        lines.push(
            "Your user PATH could not be read, so it is unknown whether the command folder is on it."
                .to_owned(),
        );
    }
    if operation == Operation::Install && report.changed && platform == Platform::Windows {
        lines.push("Open a new terminal to use massorca.".to_owned());
    }
    lines
}

/// Runs one `cli` subcommand in this process's environment and prints its outcome.
pub fn run(command: CliCommand, output: Output) -> Result<(), CliError> {
    let environment = Environment::from_process()?;
    let (operation, report) = match command {
        CliCommand::Install => (Operation::Install, install(&environment)?),
        CliCommand::Uninstall => (Operation::Uninstall, uninstall(&environment)?),
        CliCommand::Status => (Operation::Status, status(&environment)?),
    };
    output.success(&report, &lines(operation, &report, Platform::current()));
    Ok(())
}

/// What is at the command path now. A command for a binary that is no longer there is `Gone`; a command for `target`
/// itself never is, because the binary running now is there.
fn observe_command(location: &Layout, target: &Path) -> io::Result<Observed> {
    Ok(match command_file::observe(location)? {
        Observed::Ours { points_to }
            if !same_path(location.platform, &points_to, target) && !points_to.exists() =>
        {
            Observed::Gone { points_to }
        }
        observed => observed,
    })
}

/// The state of the command as it is now, and whether this run changed anything.
fn report(environment: &Environment, changed: bool) -> Result<Report, CliError> {
    let location = environment.layout();
    let observed = observe_command(location, environment.target())?;
    let on_path = environment.bin_dir_on_path();
    let points_to = match &observed {
        Observed::Ours { points_to } | Observed::Gone { points_to } => {
            Some(points_to.display().to_string())
        }
        Observed::Missing | Observed::Foreign => None,
    };
    Ok(Report {
        state: plan::classify(&observed, environment.target(), location.platform, on_path),
        method: match location.platform {
            Platform::Unix => "symlink",
            Platform::Windows => "command_file",
        },
        command: location.command_path.display().to_string(),
        target: environment.target().display().to_string(),
        points_to,
        bin_dir: location.bin_dir.display().to_string(),
        bin_dir_on_path: on_path,
        changed,
    })
}

/// Adds the command folder to the user PATH unless it is already there; returns whether the PATH changed.
fn add_bin_dir_to_user_path(
    store: &dyn UserPath,
    variables: &[(String, String)],
    value: &PathValue,
    location: &Layout,
) -> Result<bool, CliError> {
    if path_entries::contains(Platform::Windows, &value.text, &location.bin_dir, variables) {
        return Ok(false);
    }
    let text = path_entries::with_entry(Platform::Windows, &value.text, &location.bin_dir);
    store
        .write(&PathValue {
            text,
            expandable: value.expandable,
        })
        .map_err(|error| path_failed(error, &location.command_path))?;
    Ok(true)
}

/// Removes the command folder and its user PATH entry, but only once the folder holds nothing else.
fn clear_bin_dir_from_user_path(
    store: &dyn UserPath,
    variables: &[(String, String)],
    location: &Layout,
) -> Result<bool, CliError> {
    if !command_file::remove_bin_dir_if_empty(location)? {
        return Ok(false);
    }
    let value = store.read()?;
    if !path_entries::contains(Platform::Windows, &value.text, &location.bin_dir, variables) {
        return Ok(false);
    }
    let text =
        path_entries::without_entry(Platform::Windows, &value.text, &location.bin_dir, variables);
    store.write(&PathValue {
        text,
        expandable: value.expandable,
    })?;
    Ok(true)
}

/// The binary this process runs. Unix resolves links: macOS reports the path it was started by, which may be the
/// link this command manages.
fn current_binary() -> Result<PathBuf, CliError> {
    let exe = std::env::current_exe()?;
    match Platform::current() {
        Platform::Unix => Ok(std::fs::canonicalize(exe)?),
        Platform::Windows => Ok(exe),
    }
}

/// An environment variable that must name an absolute folder, which the command is kept in.
fn absolute_env_dir(name: &str) -> Result<PathBuf, CliError> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .ok_or_else(|| {
            CliError::new(
                NO_HOME,
                format!(
                    "{name} is not set to an absolute folder, so the massorca command has no folder to live in"
                ),
                exit::ERROR,
            )
        })
}

#[cfg(windows)]
fn system_user_path() -> Box<dyn UserPath> {
    Box::new(windows_user_path::WindowsUserPath)
}

#[cfg(not(windows))]
fn system_user_path() -> Box<dyn UserPath> {
    Box::new(user_path::NoUserPath)
}

fn conflict(location: &Layout, operation: Operation) -> CliError {
    let command = location.command_path.display();
    let message = match operation {
        Operation::Uninstall => {
            format!("{command} was not made by `massorca cli install`, so it was left in place")
        }
        Operation::Install | Operation::Status => format!(
            "{command} already exists and was not made by `massorca cli install`; move it aside, then run install again"
        ),
    };
    CliError::new(CONFLICT, message, exit::ERROR)
}

fn invalid_target(target: &Path) -> CliError {
    CliError::new(
        INVALID_TARGET,
        format!(
            "{} cannot be used for the command: it contains a quote, a percent sign or a line break",
            target.display()
        ),
        exit::ERROR,
    )
}

/// The command is in place when its PATH entry fails, so the message says that first.
fn path_failed(error: CliError, command: &Path) -> CliError {
    CliError {
        message: format!(
            "{} is installed, but it is not on your user PATH: {}",
            command.display(),
            error.message
        ),
        ..error
    }
}

fn off_path_hint(platform: Platform) -> String {
    match platform {
        Platform::Unix => {
            "Add that folder to PATH in your shell profile, then open a new terminal.".to_owned()
        }
        Platform::Windows => "Run `massorca cli install` to add it to your user PATH.".to_owned(),
    }
}

#[cfg(test)]
mod tests;
