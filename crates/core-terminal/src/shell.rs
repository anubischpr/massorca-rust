//! The shell a terminal runs and the environment it runs with (decision 0032).

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// A copy of environment variables. `Debug` shows names only: values can be secrets.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct EnvSnapshot {
    vars: Vec<(String, String)>,
}

impl fmt::Debug for EnvSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.vars.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("EnvSnapshot")
            .field("names", &names)
            .finish()
    }
}

impl EnvSnapshot {
    /// The service's own environment. Entries that are not valid UTF-8 are skipped.
    pub fn current() -> Self {
        Self::from_pairs(std::env::vars_os().filter_map(|(name, value)| {
            Some((name.into_string().ok()?, value.into_string().ok()?))
        }))
    }

    /// Builds a snapshot from name and value pairs; the first of a repeated name wins.
    pub fn from_pairs<I, K, V>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let vars = pairs
            .into_iter()
            .map(|(name, value)| (name.into(), value.into()))
            .filter(|(name, _): &(String, String)| !name.is_empty() && !name.starts_with('='))
            .collect();
        Self { vars }
    }

    /// The value of `name`; case-insensitive on Windows.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.vars
            .iter()
            .find(|(key, _)| names_match(key, name))
            .map(|(_, value)| value.as_str())
    }
}

/// Whether two variable names are the same: case-insensitive on Windows only.
fn names_match(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    #[error("no usable shell was found")]
    NotFound,
}

/// The program a terminal starts when the client names none (0032).
/// Windows: `pwsh.exe`, then `powershell.exe` on `PATH`, then Windows PowerShell under `SystemRoot`.
/// Unix: `$SHELL` when it is an absolute executable, then `/bin/zsh`, `/bin/bash`, `/bin/sh`.
pub fn default_shell(env: &EnvSnapshot) -> Result<PathBuf, ShellError> {
    #[cfg(windows)]
    {
        for name in ["pwsh.exe", "powershell.exe"] {
            if let Some(path) = resolve_program(name, env).filter(|path| path.is_absolute()) {
                return Ok(path);
            }
        }
        if let Some(root) = env.get("SystemRoot") {
            let path = Path::new(root)
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe");
            if is_file(&path) {
                return Ok(path);
            }
        }
        Err(ShellError::NotFound)
    }
    #[cfg(not(windows))]
    {
        if let Some(shell) = env.get("SHELL") {
            let path = Path::new(shell);
            if path.is_absolute() && is_executable(path) {
                return Ok(path.to_path_buf());
            }
        }
        ["/bin/zsh", "/bin/bash", "/bin/sh"]
            .into_iter()
            .map(PathBuf::from)
            .find(|path| is_executable(path))
            .ok_or(ShellError::NotFound)
    }
}

/// Finds the file a program name refers to. A name with a separator is taken as a path;
/// a bare name is searched in `PATH`, with `PATHEXT` extensions on Windows when it has none.
pub fn resolve_program(name: &str, env: &EnvSnapshot) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }
    if Path::new(name).is_absolute() || has_separator(name) {
        let path = Path::new(name);
        return is_file(path).then(|| path.to_path_buf());
    }
    let search_path = env.get("PATH")?;
    let candidates = candidate_names(name, env);
    std::env::split_paths(search_path)
        .flat_map(|dir| candidates.iter().map(move |candidate| dir.join(candidate)))
        .find(|path| is_file(path))
}

fn has_separator(name: &str) -> bool {
    name.contains('/') || (cfg!(windows) && name.contains('\\'))
}

/// The file names to try for a bare program name.
fn candidate_names(name: &str, env: &EnvSnapshot) -> Vec<String> {
    if !cfg!(windows) || Path::new(name).extension().is_some() {
        return vec![name.to_owned()];
    }
    let extensions = env.get("PATHEXT").unwrap_or(".COM;.EXE;.BAT;.CMD");
    extensions
        .split(';')
        .filter(|extension| !extension.is_empty())
        .map(|extension| format!("{name}{extension}"))
        .collect()
}

fn is_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

#[cfg(not(windows))]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

/// What a terminal knows about itself, for its environment.
#[derive(Debug, Clone)]
pub struct TerminalEnv<'a> {
    pub terminal_id: &'a str,
    pub workspace_id: &'a str,
    pub service_version: &'a str,
    /// The service's profile home, so `massorca` typed inside the terminal reaches the same profile.
    pub massorca_home: Option<&'a str>,
    pub massorca_profile: Option<&'a str>,
    /// Caller variables, applied last; they may override anything set here.
    pub extra: &'a BTreeMap<String, crate::EnvValue>,
}

/// Variables removed from the inherited environment: colour switches that would
/// change how the shell's tools paint.
const REMOVED: [&str; 4] = ["NO_COLOR", "FORCE_COLOR", "CLICOLOR", "CLICOLOR_FORCE"];

/// The full environment for a terminal's program, sorted by name (0032). Values are never logged.
pub fn build_environment(
    base: &EnvSnapshot,
    terminal: &TerminalEnv,
) -> Vec<(String, crate::EnvValue)> {
    let mut vars: BTreeMap<String, (String, crate::EnvValue)> = BTreeMap::new();
    for (name, value) in &base.vars {
        if !REMOVED.iter().any(|removed| names_match(removed, name)) {
            vars.insert(
                key(name),
                (name.clone(), crate::EnvValue::new(value.clone())),
            );
        }
    }
    let mut set = |name: &str, value: &str| {
        vars.insert(
            key(name),
            (name.to_owned(), crate::EnvValue::new(value.to_owned())),
        );
    };
    set("TERM", "xterm-256color");
    set("COLORTERM", "truecolor");
    set("TERM_PROGRAM", "MassOrca");
    set("TERM_PROGRAM_VERSION", terminal.service_version);
    set("MASSORCA_TERMINAL_ID", terminal.terminal_id);
    set("MASSORCA_WORKSPACE_ID", terminal.workspace_id);
    if let Some(home) = terminal.massorca_home {
        set("MASSORCA_HOME", home);
    }
    if let Some(profile) = terminal.massorca_profile {
        set("MASSORCA_PROFILE", profile);
    }
    for (name, value) in terminal.extra {
        set(name, value.as_str());
    }
    vars.into_values().collect()
}

/// The map key for a variable name: Windows names are case-insensitive.
fn key(name: &str) -> String {
    if cfg!(windows) {
        name.to_ascii_uppercase()
    } else {
        name.to_owned()
    }
}
