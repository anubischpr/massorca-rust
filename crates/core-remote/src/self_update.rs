//! The host side of a self-update over the paired link (decision 0132): runs the staged binary's `--version`, swaps it in
//! keeping the old one as `.previous`, and asks the supervisor to restart the service.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::bootstrap::UNIT_NAME;
use crate::process::{CaptureError, excerpt, run_captured};

/// How long the staged binary may take to answer `--version`.
pub const VERSION_TIMEOUT: Duration = Duration::from_secs(10);
/// How long `systemctl --no-block` or a test restart program may take.
const RESTART_TIMEOUT: Duration = Duration::from_secs(10);
/// Spawn attempts while another thread's fork still holds the freshly written file open (ETXTBSY).
#[cfg(unix)]
const BUSY_ATTEMPTS: u32 = 20;

/// What restarts the service once its binary is replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Supervisor {
    /// The systemd user unit `massorca.service` that `host add-ssh` installs (decision 0045).
    Systemd,
    /// Tests only: runs `<program> restart <installed binary>` instead of systemctl.
    Program(PathBuf),
}

impl Supervisor {
    /// The supervisor this process runs under: systemd when its cgroup is the unit's. `None` everywhere else.
    pub fn detect() -> Option<Self> {
        let cgroup = std::fs::read_to_string("/proc/self/cgroup").ok()?;
        runs_under_unit(&cgroup).then_some(Self::Systemd)
    }

    /// The name clients are told.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Systemd => "systemd",
            Self::Program(_) => "test",
        }
    }

    /// Asks for a restart; with systemd it returns once the job is queued, since the stop ends this process.
    pub fn restart(&self, installed: &Path) -> Result<(), String> {
        let command = match self {
            Self::Systemd => {
                let mut command = massorca_process::command("systemctl");
                command.args(["--user", "--no-block", "restart", UNIT_NAME]);
                command
            }
            Self::Program(program) => {
                let mut command = massorca_process::command(program);
                command.arg("restart").arg(installed);
                command
            }
        };
        let captured =
            run_captured(command, None, RESTART_TIMEOUT).map_err(|error| describe(&error))?;
        if captured.status.success() {
            Ok(())
        } else {
            Err(format!(
                "the restart failed ({}): {}",
                captured.status,
                excerpt(&captured.stderr)
            ))
        }
    }
}

/// Whether a `/proc/self/cgroup` text places this process in the unit `massorca.service`.
pub fn runs_under_unit(cgroup: &str) -> bool {
    let unit = format!("/{UNIT_NAME}");
    cgroup
        .lines()
        .filter_map(|line| line.rsplit_once(':').map(|(_, path)| path))
        .any(|path| path.ends_with(&unit))
}

/// The paths of an update next to the installed binary: same folder, so every rename stays on one filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdatePaths {
    pub installed: PathBuf,
    /// Where an upload is written until it is checked.
    pub staged: PathBuf,
    /// The binary an update replaced, kept for a rollback.
    pub previous: PathBuf,
}

impl UpdatePaths {
    /// `massorca` gives `massorca.update` and `massorca.previous`; `massorca.exe` gives `massorca.update.exe` and so on, so
    /// Windows still runs them.
    pub fn of(installed: &Path) -> Self {
        let stem = installed.file_stem().unwrap_or_default();
        let sibling = |middle: &str| {
            let mut name = OsString::from(stem);
            name.push(middle);
            if let Some(extension) = installed.extension() {
                name.push(".");
                name.push(extension);
            }
            installed.with_file_name(name)
        };
        Self {
            installed: installed.to_path_buf(),
            staged: sibling(".update"),
            previous: sibling(".previous"),
        }
    }
}

/// Runs `binary --version` and returns its first stdout line. Fails when it does not start, times out or exits non-zero.
pub fn probe_version(binary: &Path) -> Result<String, String> {
    let captured = spawn_settled(|| {
        let mut command = massorca_process::command(binary);
        command.arg("--version");
        run_captured(command, None, VERSION_TIMEOUT)
    })
    .map_err(|error| describe(&error))?;
    if !captured.status.success() {
        return Err(format!("--version exited with {}", captured.status));
    }
    Ok(String::from_utf8_lossy(&captured.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned())
}

/// Puts `staged` in place of the installed binary. The installed one is first linked (or copied) to `.previous`, then
/// the staged file is renamed over it, so the installed path never goes missing.
pub fn swap_in(paths: &UpdatePaths) -> io::Result<()> {
    match std::fs::remove_file(&paths.previous) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    if std::fs::hard_link(&paths.installed, &paths.previous).is_err() {
        std::fs::copy(&paths.installed, &paths.previous)?;
    }
    std::fs::rename(&paths.staged, &paths.installed)
}

/// Puts `.previous` back in place of the installed binary, which is discarded.
pub fn roll_back(paths: &UpdatePaths) -> io::Result<()> {
    std::fs::rename(&paths.previous, &paths.installed)
}

fn describe(error: &CaptureError) -> String {
    match error {
        CaptureError::Spawn(error) => format!("it does not run on this host: {error}"),
        CaptureError::Wait(error) => format!("it could not be waited on: {error}"),
        CaptureError::TimedOut => "it did not answer in time".to_owned(),
    }
}

/// Retries a spawn that failed with ETXTBSY: a fork elsewhere in the service briefly holds the new file open.
#[cfg(unix)]
fn spawn_settled<T>(mut run: impl FnMut() -> Result<T, CaptureError>) -> Result<T, CaptureError> {
    const ETXTBSY: i32 = 26;
    for _ in 1..BUSY_ATTEMPTS {
        match run() {
            Err(CaptureError::Spawn(error)) if error.raw_os_error() == Some(ETXTBSY) => {
                std::thread::sleep(Duration::from_millis(50));
            }
            other => return other,
        }
    }
    run()
}

#[cfg(not(unix))]
fn spawn_settled<T>(run: impl FnOnce() -> Result<T, CaptureError>) -> Result<T, CaptureError> {
    run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_unit_cgroup_counts_as_systemd() {
        let unit = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/massorca.service\n";
        assert!(runs_under_unit(unit));
        assert!(!runs_under_unit(
            "0::/user.slice/user-1000.slice/session-3.scope\n"
        ));
        assert!(!runs_under_unit("0::/app.slice/other-massorca.service\n"));
    }

    #[test]
    fn update_paths_sit_beside_the_binary_and_keep_its_extension() {
        let unix = UpdatePaths::of(Path::new("/home/deploy/.massorca/bin/massorca"));
        assert_eq!(
            unix.staged,
            Path::new("/home/deploy/.massorca/bin/massorca.update")
        );
        assert_eq!(
            unix.previous,
            Path::new("/home/deploy/.massorca/bin/massorca.previous")
        );
        let windows = UpdatePaths::of(Path::new("bin/massorca.exe"));
        assert_eq!(windows.staged, Path::new("bin/massorca.update.exe"));
        assert_eq!(windows.previous, Path::new("bin/massorca.previous.exe"));
    }

    #[test]
    fn a_swap_keeps_the_old_binary_and_a_rollback_restores_it() {
        let dir = tempfile::tempdir().expect("a folder");
        let paths = UpdatePaths::of(&dir.path().join("massorca"));
        std::fs::write(&paths.installed, b"old").expect("old");
        std::fs::write(&paths.staged, b"new").expect("new");
        swap_in(&paths).expect("swap");
        assert_eq!(std::fs::read(&paths.installed).expect("installed"), b"new");
        assert_eq!(std::fs::read(&paths.previous).expect("previous"), b"old");
        assert!(!paths.staged.exists());
        roll_back(&paths).expect("rollback");
        assert_eq!(std::fs::read(&paths.installed).expect("installed"), b"old");
        assert!(!paths.previous.exists());
    }
}
