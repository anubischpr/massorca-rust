//! Program resolution (decision 0050): a kind is installed when one of its program candidates names a file.
//!
//! The search inputs are passed in, so tests can use their own `PATH` and `PATHEXT`.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// Extensions Windows tries when `PATHEXT` is not set.
const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// What a bare program name is searched in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchEnv {
    pub path: Option<OsString>,
    /// Used on Windows only.
    pub pathext: Option<String>,
}

impl SearchEnv {
    /// The current process's `PATH` and `PATHEXT`.
    pub fn current() -> Self {
        Self {
            path: std::env::var_os("PATH"),
            pathext: std::env::var("PATHEXT").ok(),
        }
    }
}

/// The first candidate that resolves, in order.
pub fn resolve_first(candidates: &[String], env: &SearchEnv) -> Option<PathBuf> {
    candidates.iter().find_map(|name| resolve(name, env))
}

/// Finds the file `name` refers to. An absolute path must be a file (on Unix, an executable one); a bare name is
/// searched in `env.path`. Anything else (a relative path) never resolves.
pub fn resolve(name: &str, env: &SearchEnv) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }
    let path = Path::new(name);
    if path.is_absolute() {
        return runnable(path).then(|| path.to_path_buf());
    }
    if name.contains(['/', '\\']) {
        return None;
    }
    let search = env.path.as_deref()?;
    let candidates = file_names(name, env.pathext.as_deref());
    std::env::split_paths(search)
        .filter(|dir| !dir.as_os_str().is_empty())
        .flat_map(|dir| candidates.iter().map(move |file| dir.join(file)))
        .find(|candidate| runnable(candidate))
}

/// The file names to try for a bare name. On Unix only the name itself. On Windows a name whose extension is one of
/// `PATHEXT` (compared without case) is tried as is only; another name with an extension is tried as is first, then
/// with each `PATHEXT` extension appended; a name without one gets each extension.
fn file_names(name: &str, pathext: Option<&str>) -> Vec<OsString> {
    if !cfg!(windows) {
        return vec![OsString::from(name)];
    }
    let extensions: Vec<&str> = pathext
        .unwrap_or(DEFAULT_PATHEXT)
        .split(';')
        .map(str::trim)
        .filter(|extension| !extension.is_empty())
        .collect();
    let own = Path::new(name).extension().and_then(OsStr::to_str);
    let mut names = Vec::new();
    if let Some(own) = own {
        names.push(OsString::from(name));
        let executable = extensions
            .iter()
            .any(|extension| extension.trim_start_matches('.').eq_ignore_ascii_case(own));
        if executable {
            return names;
        }
    }
    names.extend(
        extensions
            .iter()
            .map(|extension| OsString::from(format!("{name}{extension}"))),
    );
    names
}

#[cfg(unix)]
fn runnable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn runnable(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// Writes an executable file `name` in `dir`.
    fn program(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    fn env_of(dirs: &[&Path], pathext: Option<&str>) -> SearchEnv {
        SearchEnv {
            path: Some(std::env::join_paths(dirs).unwrap()),
            pathext: pathext.map(str::to_owned),
        }
    }

    #[test]
    fn an_absolute_path_resolves_when_it_is_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = program(dir.path(), "agent-bin");
        let text = file.to_str().unwrap();
        assert_eq!(resolve(text, &SearchEnv::default()), Some(file.clone()));
        let missing = dir.path().join("nothing");
        assert_eq!(
            resolve(missing.to_str().unwrap(), &SearchEnv::default()),
            None
        );
        assert_eq!(
            resolve(dir.path().to_str().unwrap(), &SearchEnv::default()),
            None,
            "a directory is not a program"
        );
    }

    #[test]
    fn a_relative_path_never_resolves() {
        let dir = tempfile::tempdir().unwrap();
        program(dir.path(), "agent");
        let env = env_of(&[dir.path()], None);
        assert_eq!(resolve("./agent", &env), None);
        assert_eq!(resolve("", &env), None);
    }

    #[test]
    fn a_bare_name_is_found_in_the_first_directory_that_has_it() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let name = if cfg!(windows) { "agent.exe" } else { "agent" };
        let found = program(second.path(), name);
        let env = env_of(&[first.path(), second.path()], Some(".exe"));
        assert_eq!(resolve("agent", &env), Some(found.clone()));
        let earlier = program(first.path(), name);
        assert_eq!(resolve("agent", &env), Some(earlier));
        assert_eq!(resolve("other", &env), None);
        assert_eq!(resolve("agent", &SearchEnv::default()), None, "no PATH");
    }

    #[test]
    fn the_first_candidate_that_resolves_wins() {
        let dir = tempfile::tempdir().unwrap();
        let file = program(dir.path(), "second-bin");
        let candidates = vec![
            dir.path().join("first-bin").to_string_lossy().into_owned(),
            file.to_string_lossy().into_owned(),
        ];
        assert_eq!(
            resolve_first(&candidates, &SearchEnv::default()),
            Some(file)
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_file_without_execute_permission_is_not_a_program() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("agent"), "").unwrap();
        assert_eq!(resolve("agent", &env_of(&[dir.path()], None)), None);
    }

    #[cfg(windows)]
    #[test]
    fn windows_tries_pathext_extensions_for_a_bare_name() {
        let dir = tempfile::tempdir().unwrap();
        let cmd = program(dir.path(), "claude.cmd");
        program(dir.path(), "claude");
        let env = env_of(&[dir.path()], Some(".EXE;.CMD"));
        let found = resolve("claude", &env).unwrap();
        assert!(
            found
                .to_string_lossy()
                .eq_ignore_ascii_case(&cmd.to_string_lossy()),
            "{found:?}"
        );
        // Without PATHEXT the default list still includes .CMD.
        assert!(resolve("claude", &env_of(&[dir.path()], None)).is_some());
        // An extensionless file is never run on Windows.
        assert_eq!(
            resolve("claude", &env_of(&[dir.path()], Some(".EXE"))),
            None
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_takes_a_name_with_an_executable_extension_as_is() {
        let dir = tempfile::tempdir().unwrap();
        program(dir.path(), "codex.CMD");
        let env = env_of(&[dir.path()], Some(".exe;.cmd"));
        assert!(resolve("codex.cmd", &env).is_some());
        assert_eq!(
            file_names("codex.Cmd", Some(".exe;.cmd")),
            vec![OsString::from("codex.Cmd")]
        );
        // Another extension is tried as is first, then with PATHEXT appended.
        assert_eq!(
            file_names("tool.v2", Some(".EXE")),
            vec![OsString::from("tool.v2"), OsString::from("tool.v2.EXE")]
        );
    }
}
