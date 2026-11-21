//! Helpers shared by the `TerminalManager` tests: real shells, every wait bounded.

#![allow(
    dead_code,
    reason = "each test binary uses a different part of this module"
)]

use std::collections::BTreeMap;
use std::ops::Deref;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::shell::EnvSnapshot;
use massorca_core_terminal::{CreateSpec, ManagerConfig, ShellChoice, TerminalManager};

pub const TIMEOUT: Duration = Duration::from_secs(10);

/// Wide enough that ConPTY does not wrap the lines the tests read.
pub const SIZE: PtySize = PtySize {
    cols: 200,
    rows: 50,
};

/// A manager that kills its terminals when the test ends, failed assertions included.
pub struct Managed(pub TerminalManager);

impl Managed {
    pub fn new() -> Self {
        Self::with(|_| {})
    }

    pub fn with(change: impl FnOnce(&mut ManagerConfig)) -> Self {
        let mut config = ManagerConfig::new(EnvSnapshot::current(), "0.0.0-test".to_owned());
        change(&mut config);
        Self(TerminalManager::new(config))
    }
}

impl Deref for Managed {
    type Target = TerminalManager;

    fn deref(&self) -> &TerminalManager {
        &self.0
    }
}

impl Drop for Managed {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}

/// The platform's interactive shell: `cmd.exe` on Windows, `sh` elsewhere.
pub fn shell() -> ShellChoice {
    if cfg!(windows) {
        ShellChoice {
            program: "cmd.exe".to_owned(),
            args: vec!["/d".to_owned()],
        }
    } else {
        ShellChoice {
            program: "sh".to_owned(),
            args: Vec::new(),
        }
    }
}

/// A shell that runs `script` and exits: `cmd /d /c` or `sh -c`.
pub fn script_shell(script: &str) -> ShellChoice {
    let flag = if cfg!(windows) { "/c" } else { "-c" };
    let mut choice = shell();
    choice.args.push(flag.to_owned());
    choice.args.push(script.to_owned());
    choice
}

pub fn spec(workspace: &str, cwd: &Path) -> CreateSpec {
    CreateSpec {
        workspace_id: workspace.to_owned(),
        cwd: cwd.to_path_buf(),
        shell: Some(shell()),
        env: BTreeMap::new(),
        size: SIZE,
        title: None,
        request_id: None,
    }
}

/// Polls `done` until it holds, or panics after `TIMEOUT`.
pub fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + TIMEOUT;
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(20));
    }
}
