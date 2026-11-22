//! What the install flow needs from the host, supplied by the service: finding a program and running a command.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use massorca_ext_package::manifest::agent::CliOs;

/// How long a vendor installer, uninstaller or verify command may run before it is killed.
pub const INSTALLER_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// The cap on a command's log file.
pub const MAX_LOG_BYTES: u64 = 1024 * 1024;

/// Resolves a bare program name the way `core-agents` does: PATH, plus PATHEXT on Windows.
pub trait ProgramResolver: Send + Sync {
    fn resolve(&self, binary: &str) -> Option<PathBuf>;
}

/// How a command ended. `code` is `None` when it was killed or ended by a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exit {
    pub code: Option<i32>,
    pub timed_out: bool,
}

impl Exit {
    pub fn success(self) -> bool {
        self.code == Some(0) && !self.timed_out
    }
}

/// Runs one argv on the service host. The contract an implementation keeps:
///
/// - `argv[0]` is the program and the rest are its arguments, passed as they are: no shell, no string joining.
/// - No console window opens on Windows, and nothing is elevated by MassOrca.
/// - stdout and stderr are appended to `log`, capped at [`MAX_LOG_BYTES`].
/// - The child process is killed when `timeout` passes, and the result says `timed_out`.
pub trait CommandRunner: Send + Sync {
    fn run(&self, argv: &[String], log: &Path, timeout: Duration) -> io::Result<Exit>;
}

/// The host OS as a manifest names it, or `None` on an OS no package targets.
pub fn current_os() -> Option<CliOs> {
    match std::env::consts::OS {
        "windows" => Some(CliOs::Windows),
        "linux" => Some(CliOs::Linux),
        "macos" => Some(CliOs::Macos),
        _ => None,
    }
}

/// The manifest spelling of an OS.
pub fn os_name(os: CliOs) -> &'static str {
    match os {
        CliOs::Windows => "windows",
        CliOs::Linux => "linux",
        CliOs::Macos => "macos",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exit_zero_in_time_is_success() {
        let ok = Exit {
            code: Some(0),
            timed_out: false,
        };
        assert!(ok.success());
        assert!(
            !Exit {
                code: Some(1),
                ..ok
            }
            .success()
        );
        assert!(
            !Exit {
                timed_out: true,
                ..ok
            }
            .success()
        );
        assert!(
            current_os().is_some(),
            "tests run on windows, linux or macos"
        );
    }
}
