//! Helpers shared by the terminal integration tests.
// Each test binary compiles this module and uses only part of it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use massorca_core_terminal::pty::{PtySize, SpawnSpec};
use massorca_core_terminal::shell::{EnvSnapshot, TerminalEnv, build_environment, resolve_program};

/// Wide enough that ConPTY does not wrap or redraw the lines the tests read.
pub const SIZE: PtySize = PtySize {
    cols: 200,
    rows: 50,
};

/// The platform's non-interactive shell: `sh` on Unix, `cmd.exe` on Windows.
pub fn shell_program() -> PathBuf {
    let name = if cfg!(windows) { "cmd.exe" } else { "sh" };
    resolve_program(name, &EnvSnapshot::current()).expect("the platform shell is on PATH")
}

/// The environment a terminal really gets, built from this process's environment.
pub fn terminal_env() -> Vec<(String, String)> {
    let extra = std::collections::BTreeMap::new();
    build_environment(
        &EnvSnapshot::current(),
        &TerminalEnv {
            terminal_id: "test-terminal",
            workspace_id: "test-workspace",
            service_version: "0.0.0-test",
            massorca_home: None,
            massorca_profile: None,
            extra: &extra,
        },
    )
}

pub fn spec(program: PathBuf, args: &[&str], cwd: &Path, env: Vec<(String, String)>) -> SpawnSpec {
    SpawnSpec {
        program,
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        cwd: cwd.to_path_buf(),
        env,
        size: SIZE,
    }
}
