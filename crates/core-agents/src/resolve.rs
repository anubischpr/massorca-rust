//! Program resolution (decision 0050): a kind is installed when one of its program candidates names a file.
//!
//! The search inputs are passed in, so tests can use their own `PATH` and `PATHEXT`.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

/// Where a vendor program's search path came from (decision 0092).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathSource {
    /// The service's own `PATH`.
    Process,
    /// The service's `PATH` with the registry's machine and user `Path` values merged in.
    Registry,
}

impl PathSource {
    /// The word shown to a person: `process` or `registry`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Process => "process",
            Self::Registry => "registry",
        }
    }
}

/// The search path of vendor programs (decision 0092): on Windows the process `PATH` with the registry's machine and
/// user `Path` values merged in; elsewhere the process `PATH`.
pub fn vendor_search_env() -> (SearchEnv, PathSource) {
    let process = std::env::var_os("PATH");
    let pathext = std::env::var("PATHEXT").ok();
    let merged = if process_path_only() {
        None
    } else {
        registry_merged_path(process.as_deref())
    };
    match merged {
        Some(merged) => (
            SearchEnv {
                path: Some(OsString::from(merged)),
                pathext,
            },
            PathSource::Registry,
        ),
        None => (
            SearchEnv {
                path: process,
                pathext,
            },
            PathSource::Process,
        ),
    }
}

/// Where agent programs are searched for when kinds are listed and started (decision 0164): the host's search path, the
/// one the extension install check uses (decision 0092), or a fixed one for tests and support.
#[derive(Clone)]
pub struct VendorSearch(Arc<dyn Fn() -> SearchEnv + Send + Sync>);

impl VendorSearch {
    /// The host's search path: the process `PATH`, with the registry's machine and user `Path` merged in on Windows.
    pub fn host() -> Self {
        Self(Arc::new(|| vendor_search_env().0))
    }

    /// A search that always gives `env`.
    pub fn fixed(env: SearchEnv) -> Self {
        Self(Arc::new(move || env.clone()))
    }

    /// The search path now.
    pub fn env(&self) -> SearchEnv {
        (self.0)()
    }
}

impl std::fmt::Debug for VendorSearch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VendorSearch")
    }
}

/// Whether vendor programs use the service's own `PATH` alone: `MASSORCA_VENDOR_PATH=process`, for tests and support.
fn process_path_only() -> bool {
    std::env::var_os("MASSORCA_VENDOR_PATH").is_some_and(|value| value.to_str() == Some("process"))
}

/// The process `PATH`, then the machine and user `Path` values (decision 0092). Entries are trimmed and unquoted, empty
/// ones are dropped, and a repeat is dropped by its first occurrence, compared without case and trailing separators as
/// Windows compares directories. The registry values must already have their `%NAME%` names expanded.
pub fn merge_search_path(process: &str, machine: Option<&str>, user: Option<&str>) -> String {
    let mut seen = HashSet::new();
    let mut kept = Vec::new();
    let entries = process
        .split(';')
        .chain(machine.unwrap_or_default().split(';'))
        .chain(user.unwrap_or_default().split(';'));
    for entry in entries {
        let entry = entry.trim().trim_matches('"').trim();
        if entry.is_empty() {
            continue;
        }
        let key = entry.trim_end_matches(['\\', '/']).to_ascii_lowercase();
        if seen.insert(key) {
            kept.push(entry);
        }
    }
    kept.join(";")
}

/// Replaces each `%NAME%` that `lookup` knows with its value. A name it does not know, or a lone `%`, stays as typed.
pub fn expand_env_vars(value: &str, lookup: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('%').filter(|end| *end > 0) else {
            break;
        };
        out.push_str(&rest[..start]);
        let name = &after[..end];
        match lookup(name) {
            Some(found) => out.push_str(&found),
            None => out.push_str(&rest[start..start + end + 2]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The process `PATH` merged with the registry's `Path` values, or `None` when the registry has neither value, or the
/// process `PATH` is not text.
#[cfg(windows)]
fn registry_merged_path(process: Option<&OsStr>) -> Option<String> {
    let process = process?.to_str()?;
    let machine = registry::path_value(registry::MACHINE_PATH);
    let user = registry::path_value(registry::USER_PATH);
    if machine.is_none() && user.is_none() {
        return None;
    }
    Some(merge_search_path(
        process,
        machine.as_deref(),
        user.as_deref(),
    ))
}

#[cfg(not(windows))]
fn registry_merged_path(_process: Option<&OsStr>) -> Option<String> {
    None
}

/// The machine and user `Path` values, read through the Windows registry API: no `reg.exe` and no PowerShell.
#[cfg(windows)]
#[allow(unsafe_code)]
mod registry {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, REG_EXPAND_SZ, REG_VALUE_TYPE,
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
    };

    use super::expand_env_vars;

    /// The `Path` value of the machine's environment.
    pub(super) const MACHINE_PATH: (HKEY, &str) = (
        HKEY_LOCAL_MACHINE,
        r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
    );
    /// The `Path` value of the signed-in user's environment.
    pub(super) const USER_PATH: (HKEY, &str) = (HKEY_CURRENT_USER, "Environment");

    /// The `Path` value under `root` and `subkey`, with `%NAME%` expanded when it is `REG_EXPAND_SZ`; `None` if missing.
    pub(super) fn path_value((root, subkey): (HKEY, &str)) -> Option<String> {
        let subkey = wide(subkey);
        let name = wide("Path");
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: `subkey` is NUL-terminated and lives across the call; on success `key` is open until `Key` closes it.
        let status = unsafe { RegOpenKeyExW(root, subkey.as_ptr(), 0, KEY_READ, &mut key) };
        if status != ERROR_SUCCESS {
            return None;
        }
        let key = Key(key);
        let mut kind: REG_VALUE_TYPE = 0;
        let mut bytes: u32 = 0;
        // SAFETY: with no data buffer the call only reports the size of the value into `bytes`.
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                name.as_ptr(),
                std::ptr::null(),
                &mut kind,
                std::ptr::null_mut(),
                &mut bytes,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let mut units = vec![0_u16; (bytes as usize).div_ceil(2)];
        let mut size = bytes;
        // SAFETY: `units` holds `size` bytes or more, and the call writes no more than `size` bytes into it.
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                name.as_ptr(),
                std::ptr::null(),
                &mut kind,
                units.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let used = (size as usize).div_ceil(2).min(units.len());
        let text = String::from_utf16_lossy(&units[..used]);
        let text = text.trim_end_matches('\0');
        Some(if kind == REG_EXPAND_SZ {
            expand_env_vars(text, &|name: &str| std::env::var(name).ok())
        } else {
            text.to_owned()
        })
    }

    /// An open registry key, closed when dropped.
    struct Key(HKEY);

    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: the handle came from a successful `RegOpenKeyExW` and is closed only here.
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        OsStr::new(text)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
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

    #[test]
    fn expansion_replaces_known_names_and_keeps_the_rest_as_typed() {
        let lookup = |name: &str| (name == "ROOT").then(|| "C:\\Root".to_owned());
        assert_eq!(
            expand_env_vars("%ROOT%\\bin;%MISSING%\\x", &lookup),
            "C:\\Root\\bin;%MISSING%\\x"
        );
        assert_eq!(expand_env_vars("100% sure", &lookup), "100% sure");
        assert_eq!(expand_env_vars("%ROOT", &lookup), "%ROOT");
    }

    #[test]
    fn the_merge_keeps_process_then_machine_then_user_and_drops_repeats() {
        let merged = merge_search_path(
            "C:\\Tools;C:\\Node\\",
            Some("C:\\Windows;c:\\tools;C:\\Node"),
            Some("C:\\Users\\me\\npm;C:\\Windows"),
        );
        assert_eq!(
            merged,
            "C:\\Tools;C:\\Node\\;C:\\Windows;C:\\Users\\me\\npm"
        );
    }

    #[test]
    fn the_merge_skips_empty_entries_and_unquotes_quoted_ones() {
        assert_eq!(
            merge_search_path("", Some("\"C:\\Program Files\\nodejs\";;"), None),
            "C:\\Program Files\\nodejs"
        );
        assert_eq!(merge_search_path("C:\\a", None, None), "C:\\a");
    }

    /// A bundle installer's `.cmd` launcher sits in a folder only the registry's user `Path` lists (decision 0092). The
    /// merged search finds it under Windows' PATHEXT; the process `PATH` alone does not.
    #[cfg(windows)]
    #[test]
    fn a_launcher_on_the_registry_user_path_resolves_through_the_merged_search() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let bundle = dir.path().join("bundle").join("bin");
        let process = dir.path().join("process");
        fs::create_dir_all(&bundle).expect("a bundle folder");
        fs::create_dir_all(&process).expect("a process folder");
        fs::write(bundle.join("gentle-shell.cmd"), "@echo off\r\n").expect("a launcher");
        let user = bundle.display().to_string();
        let process_text = process.display().to_string();
        let pathext = Some(".COM;.EXE;.BAT;.CMD".to_owned());
        let merged = SearchEnv {
            path: Some(OsString::from(merge_search_path(
                &process_text,
                None,
                Some(&user),
            ))),
            pathext: pathext.clone(),
        };
        // The PATHEXT spelling decides the file's case, and Windows names the same file either way.
        let found = resolve("gentle-shell", &merged).expect("the launcher resolves");
        assert!(
            found
                .to_string_lossy()
                .eq_ignore_ascii_case(&bundle.join("gentle-shell.cmd").to_string_lossy()),
            "{found:?}"
        );
        let process_only = SearchEnv {
            path: Some(OsString::from(&process_text)),
            pathext,
        };
        assert_eq!(resolve("gentle-shell", &process_only), None);
    }
}
