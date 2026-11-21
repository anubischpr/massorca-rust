//! Environment building, program resolution and the default shell (decision 0032).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use massorca_core_terminal::shell::{
    EnvSnapshot, TerminalEnv, build_environment, default_shell, resolve_program,
};

fn path_value(dirs: &[&Path]) -> String {
    std::env::join_paths(dirs)
        .expect("join paths")
        .into_string()
        .expect("utf-8 path")
}

fn terminal_env(extra: &BTreeMap<String, String>) -> TerminalEnv<'_> {
    TerminalEnv {
        terminal_id: "terminal-1",
        workspace_id: "workspace-1",
        service_version: "9.8.7",
        massorca_home: Some("/profile/home"),
        massorca_profile: Some("work"),
        extra,
    }
}

fn value_of(env: &[(String, String)], name: &str) -> Option<String> {
    env.iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.clone())
}

#[test]
fn terminal_identity_and_colour_variables_are_set() {
    let base = EnvSnapshot::from_pairs([("PATH", "/bin")]);
    let extra = BTreeMap::new();
    let env = build_environment(&base, &terminal_env(&extra));

    assert_eq!(value_of(&env, "TERM").as_deref(), Some("xterm-256color"));
    assert_eq!(value_of(&env, "COLORTERM").as_deref(), Some("truecolor"));
    assert_eq!(value_of(&env, "TERM_PROGRAM").as_deref(), Some("MassOrca"));
    assert_eq!(
        value_of(&env, "TERM_PROGRAM_VERSION").as_deref(),
        Some("9.8.7")
    );
    assert_eq!(
        value_of(&env, "MASSORCA_TERMINAL_ID").as_deref(),
        Some("terminal-1")
    );
    assert_eq!(
        value_of(&env, "MASSORCA_WORKSPACE_ID").as_deref(),
        Some("workspace-1")
    );
    assert_eq!(
        value_of(&env, "MASSORCA_HOME").as_deref(),
        Some("/profile/home")
    );
    assert_eq!(value_of(&env, "MASSORCA_PROFILE").as_deref(), Some("work"));
    assert_eq!(value_of(&env, "PATH").as_deref(), Some("/bin"));
}

#[test]
fn colour_switches_are_removed_and_other_variables_kept() {
    let base = EnvSnapshot::from_pairs([
        ("NO_COLOR", "1"),
        ("FORCE_COLOR", "1"),
        ("CLICOLOR", "1"),
        ("CLICOLOR_FORCE", "1"),
        ("HOME_MARKER", "kept"),
    ]);
    let extra = BTreeMap::new();
    let env = build_environment(&base, &terminal_env(&extra));

    for removed in ["NO_COLOR", "FORCE_COLOR", "CLICOLOR", "CLICOLOR_FORCE"] {
        assert_eq!(value_of(&env, removed), None, "{removed} must be removed");
    }
    assert_eq!(value_of(&env, "HOME_MARKER").as_deref(), Some("kept"));
}

#[test]
fn caller_variables_are_applied_last_and_may_override() {
    let base = EnvSnapshot::from_pairs([("TERM", "linux"), ("MASSORCA_HOME", "/old")]);
    let mut extra = BTreeMap::new();
    extra.insert("TERM".to_owned(), "dumb".to_owned());
    extra.insert("NO_COLOR".to_owned(), "1".to_owned());
    extra.insert("CUSTOM_FLAG".to_owned(), "on".to_owned());
    let env = build_environment(&base, &terminal_env(&extra));

    assert_eq!(value_of(&env, "TERM").as_deref(), Some("dumb"));
    assert_eq!(value_of(&env, "NO_COLOR").as_deref(), Some("1"));
    assert_eq!(value_of(&env, "CUSTOM_FLAG").as_deref(), Some("on"));
    assert_eq!(
        value_of(&env, "MASSORCA_HOME").as_deref(),
        Some("/profile/home")
    );
}

#[test]
fn profile_variables_keep_the_inherited_value_when_not_given() {
    let base =
        EnvSnapshot::from_pairs([("MASSORCA_HOME", "/inherited"), ("MASSORCA_PROFILE", "p")]);
    let extra = BTreeMap::new();
    let mut env_without = terminal_env(&extra);
    env_without.massorca_home = None;
    env_without.massorca_profile = None;
    let env = build_environment(&base, &env_without);

    assert_eq!(
        value_of(&env, "MASSORCA_HOME").as_deref(),
        Some("/inherited")
    );
    assert_eq!(value_of(&env, "MASSORCA_PROFILE").as_deref(), Some("p"));
}

#[test]
fn output_is_sorted_by_name_and_the_same_for_the_same_input() {
    let forward = EnvSnapshot::from_pairs([("ZED", "1"), ("ALPHA", "2"), ("MID", "3")]);
    let reverse = EnvSnapshot::from_pairs([("MID", "3"), ("ALPHA", "2"), ("ZED", "1")]);
    let extra = BTreeMap::new();
    let first = build_environment(&forward, &terminal_env(&extra));
    let second = build_environment(&reverse, &terminal_env(&extra));

    assert_eq!(first, second);
    let names: Vec<&str> = first.iter().map(|(name, _)| name.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
}

#[test]
fn a_repeated_name_keeps_one_entry() {
    let base = EnvSnapshot::from_pairs([("TERM", "linux")]);
    let extra = BTreeMap::new();
    let env = build_environment(&base, &terminal_env(&extra));
    let count = env.iter().filter(|(name, _)| name == "TERM").count();
    assert_eq!(count, 1);
}

#[cfg(windows)]
#[test]
fn windows_variable_names_compare_case_insensitively() {
    let base = EnvSnapshot::from_pairs([("no_color", "1"), ("term", "linux")]);
    let mut extra = BTreeMap::new();
    extra.insert("Term_Program".to_owned(), "other".to_owned());
    let env = build_environment(&base, &terminal_env(&extra));

    assert_eq!(value_of(&env, "no_color"), None);
    assert_eq!(
        env.iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case("TERM"))
            .count(),
        1
    );
    assert_eq!(value_of(&env, "Term_Program").as_deref(), Some("other"));
    assert!(
        EnvSnapshot::from_pairs([("Path", "x")])
            .get("PATH")
            .is_some()
    );
}

#[cfg(not(windows))]
#[test]
fn unix_variable_names_are_case_sensitive() {
    let base = EnvSnapshot::from_pairs([("no_color", "1")]);
    assert_eq!(base.get("NO_COLOR"), None);
    let extra = BTreeMap::new();
    let env = build_environment(&base, &terminal_env(&extra));
    assert_eq!(value_of(&env, "no_color").as_deref(), Some("1"));
}

#[test]
fn snapshot_get_finds_names_and_debug_hides_values() {
    let snapshot = EnvSnapshot::from_pairs([("SECRET_TOKEN", "do-not-print")]);
    assert_eq!(snapshot.get("SECRET_TOKEN"), Some("do-not-print"));
    assert_eq!(snapshot.get("MISSING"), None);
    let debug = format!("{snapshot:?}");
    assert!(debug.contains("SECRET_TOKEN"));
    assert!(!debug.contains("do-not-print"));
}

#[test]
fn resolve_program_finds_a_file_on_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = dir
        .path()
        .join(if cfg!(windows) { "tool.exe" } else { "tool" });
    std::fs::write(&file, b"").expect("write");
    let env = EnvSnapshot::from_pairs([("PATH", path_value(&[dir.path()]))]);

    let name = file
        .file_name()
        .expect("name")
        .to_string_lossy()
        .into_owned();
    assert_eq!(resolve_program(&name, &env), Some(file));
}

#[test]
fn resolve_program_returns_none_when_missing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let env = EnvSnapshot::from_pairs([("PATH", path_value(&[dir.path()]))]);
    assert_eq!(resolve_program("massorca-no-such-tool", &env), None);
    assert_eq!(resolve_program("", &env), None);
    let without_path = EnvSnapshot::from_pairs([("OTHER", "1")]);
    assert_eq!(
        resolve_program("massorca-no-such-tool", &without_path),
        None
    );
}

#[test]
fn resolve_program_takes_an_explicit_path_as_it_is() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = dir.path().join("explicit-tool");
    std::fs::write(&file, b"").expect("write");
    let env = EnvSnapshot::from_pairs([("PATH", "")]);

    assert_eq!(
        resolve_program(&file.to_string_lossy(), &env),
        Some(file.clone())
    );
    let missing: PathBuf = dir.path().join("not-there");
    assert_eq!(resolve_program(&missing.to_string_lossy(), &env), None);
    // A directory is not a program.
    assert_eq!(resolve_program(&dir.path().to_string_lossy(), &env), None);
}

#[cfg(windows)]
#[test]
fn resolve_program_tries_pathext_when_the_name_has_no_extension() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = dir.path().join("tool.cmd");
    std::fs::write(&file, b"@echo off\r\n").expect("write");
    let env = EnvSnapshot::from_pairs([
        ("PATH", path_value(&[dir.path()])),
        ("PATHEXT", ".COM;.EXE;.CMD".to_owned()),
    ]);

    // Windows file names are case-insensitive, so the PATHEXT casing may be returned.
    let found = resolve_program("tool", &env).expect("tool.cmd is found through PATHEXT");
    assert!(
        found
            .to_string_lossy()
            .eq_ignore_ascii_case(&file.to_string_lossy())
    );
    assert_eq!(resolve_program("tool.cmd", &env), Some(file));
    assert_eq!(resolve_program("tool.bat", &env), None);
}

#[cfg(windows)]
#[test]
fn default_shell_prefers_pwsh_then_powershell_on_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let powershell = dir.path().join("powershell.exe");
    std::fs::write(&powershell, b"").expect("write");
    let env = EnvSnapshot::from_pairs([("PATH", path_value(&[dir.path()]))]);
    assert_eq!(default_shell(&env).expect("shell"), powershell);

    let pwsh = dir.path().join("pwsh.exe");
    std::fs::write(&pwsh, b"").expect("write");
    assert_eq!(default_shell(&env).expect("shell"), pwsh);
}

#[cfg(windows)]
#[test]
fn default_shell_fails_when_no_powershell_exists() {
    let dir = tempfile::tempdir().expect("temp dir");
    let env = EnvSnapshot::from_pairs([
        ("PATH", path_value(&[dir.path()])),
        ("SystemRoot", dir.path().to_string_lossy().into_owned()),
    ]);
    assert!(default_shell(&env).is_err());
}

#[cfg(not(windows))]
#[test]
fn default_shell_falls_back_when_shell_is_relative() {
    use massorca_core_terminal::shell::ShellError;

    let env = EnvSnapshot::from_pairs([("SHELL", "zsh")]);
    match default_shell(&env) {
        Ok(path) => {
            assert!(path.is_absolute(), "{path:?}");
            assert!(
                ["/bin/zsh", "/bin/bash", "/bin/sh"]
                    .iter()
                    .any(|candidate| path == Path::new(candidate)),
                "{path:?}"
            );
        }
        Err(ShellError::NotFound) => panic!("no fallback shell found on this host"),
    }
}

#[cfg(not(windows))]
#[test]
fn default_shell_uses_an_absolute_executable_shell() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("temp dir");
    let shell = dir.path().join("my-shell");
    std::fs::write(&shell, b"#!/bin/sh\n").expect("write");
    std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let env = EnvSnapshot::from_pairs([("SHELL", shell.to_string_lossy().into_owned())]);
    assert_eq!(default_shell(&env).expect("shell"), shell);
}

#[cfg(not(windows))]
#[test]
fn default_shell_skips_a_non_executable_shell() {
    let dir = tempfile::tempdir().expect("temp dir");
    let shell = dir.path().join("plain-file");
    std::fs::write(&shell, b"not a program").expect("write");
    let env = EnvSnapshot::from_pairs([("SHELL", shell.to_string_lossy().into_owned())]);
    let chosen = default_shell(&env).expect("fallback shell");
    assert_ne!(chosen, shell);
}
