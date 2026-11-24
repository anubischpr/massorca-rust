//! Decides what install, uninstall and status do from what sits at the command path. Nothing here touches the disk.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::layout::{Platform, same_path};

/// What is at the command path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observed {
    /// Nothing is there.
    Missing,
    /// A command that `cli install` made, which runs `points_to`.
    Ours { points_to: PathBuf },
    /// Something massorca did not make.
    Foreign,
}

/// What `status` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// The command runs this binary and its folder is on PATH.
    Installed,
    /// The command runs this binary, but its folder is not on PATH.
    NotOnPath,
    /// The command runs another binary.
    DifferentBinary,
    /// No command is installed.
    NotInstalled,
    /// Something massorca did not make is at the command path.
    Conflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStep {
    Create,
    Keep,
    Repoint,
    Refuse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallStep {
    Remove,
    Nothing,
    Refuse,
}

/// The state of the command: `on_path` is `None` when the PATH could not be read.
pub fn classify(
    observed: &Observed,
    target: &Path,
    platform: Platform,
    on_path: Option<bool>,
) -> State {
    match observed {
        Observed::Missing => State::NotInstalled,
        Observed::Foreign => State::Conflict,
        Observed::Ours { points_to } if !same_path(platform, points_to, target) => {
            State::DifferentBinary
        }
        Observed::Ours { .. } if on_path == Some(false) => State::NotOnPath,
        Observed::Ours { .. } => State::Installed,
    }
}

pub fn install_step(observed: &Observed, target: &Path, platform: Platform) -> InstallStep {
    match observed {
        Observed::Missing => InstallStep::Create,
        Observed::Foreign => InstallStep::Refuse,
        Observed::Ours { points_to } if same_path(platform, points_to, target) => InstallStep::Keep,
        Observed::Ours { .. } => InstallStep::Repoint,
    }
}

pub fn uninstall_step(observed: &Observed) -> UninstallStep {
    match observed {
        Observed::Missing => UninstallStep::Nothing,
        Observed::Foreign => UninstallStep::Refuse,
        Observed::Ours { .. } => UninstallStep::Remove,
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    fn ours(path: &str) -> Observed {
        Observed::Ours {
            points_to: PathBuf::from(path),
        }
    }

    #[test]
    fn install_creates_a_missing_command_and_keeps_one_that_runs_this_binary() {
        let target = Path::new("/opt/massorca/massorca");
        assert_eq!(
            install_step(&Observed::Missing, target, Platform::Unix),
            InstallStep::Create
        );
        assert_eq!(
            install_step(&ours("/opt/massorca/massorca"), target, Platform::Unix),
            InstallStep::Keep
        );
    }

    #[test]
    fn install_repoints_a_command_for_another_binary_and_refuses_a_foreign_file() {
        let target = Path::new("/opt/massorca/massorca");
        assert_eq!(
            install_step(&ours("/opt/old/massorca"), target, Platform::Unix),
            InstallStep::Repoint
        );
        assert_eq!(
            install_step(&Observed::Foreign, target, Platform::Unix),
            InstallStep::Refuse
        );
    }

    #[test]
    fn uninstall_removes_only_what_install_made() {
        assert_eq!(
            uninstall_step(&ours("/opt/massorca/massorca")),
            UninstallStep::Remove
        );
        assert_eq!(uninstall_step(&Observed::Missing), UninstallStep::Nothing);
        assert_eq!(uninstall_step(&Observed::Foreign), UninstallStep::Refuse);
    }

    #[test]
    fn status_reports_installed_only_when_the_folder_is_on_path_or_unknown() {
        let target = Path::new("/opt/massorca/massorca");
        let command = ours("/opt/massorca/massorca");
        assert_eq!(
            classify(&command, target, Platform::Unix, Some(true)),
            State::Installed
        );
        assert_eq!(
            classify(&command, target, Platform::Unix, None),
            State::Installed
        );
        assert_eq!(
            classify(&command, target, Platform::Unix, Some(false)),
            State::NotOnPath
        );
    }

    #[test]
    fn status_names_a_different_binary_a_foreign_file_and_a_missing_command() {
        let target = Path::new("/opt/massorca/massorca");
        assert_eq!(
            classify(
                &ours("/opt/old/massorca"),
                target,
                Platform::Unix,
                Some(false)
            ),
            State::DifferentBinary
        );
        assert_eq!(
            classify(&Observed::Foreign, target, Platform::Unix, Some(true)),
            State::Conflict
        );
        assert_eq!(
            classify(&Observed::Missing, target, Platform::Unix, Some(true)),
            State::NotInstalled
        );
    }
}
