//! Stand-in tool programs for a temporary `PATH`.

use std::path::{Path, PathBuf};

use massorca_core_terminal::shell::EnvSnapshot;

/// Writes a stand-in program `name` in `dir`: a `.cmd` batch on Windows, a shell script elsewhere.
pub fn stand_in(dir: &Path, name: &str, unix_body: &str, windows_body: &str) -> PathBuf {
    if cfg!(windows) {
        let path = dir.join(format!("{name}.cmd"));
        std::fs::write(&path, format!("@echo off\r\n{windows_body}\r\n")).expect("write stand-in");
        path
    } else {
        let path = dir.join(name);
        write_executable(&path, &format!("#!/bin/sh\n{unix_body}\n"));
        path
    }
}

/// A stand-in that prints `line` for every run and appends one line to `log` per run.
pub fn versioned(dir: &Path, name: &str, line: &str, log: &Path) -> PathBuf {
    stand_in(
        dir,
        name,
        &format!("echo run >> '{}'\necho '{line}'", log.display()),
        &format!("echo run>>\"{}\"\r\necho {line}", log.display()),
    )
}

/// A `PATH` that holds only `dir`.
pub fn env_with_path(dir: &Path) -> EnvSnapshot {
    EnvSnapshot::from_pairs([("PATH", dir.to_str().expect("utf-8 path"))])
}

/// The number of runs a stand-in logged to `log`.
pub fn runs(log: &Path) -> usize {
    std::fs::read_to_string(log).map_or(0, |text| text.lines().count())
}

#[cfg(unix)]
fn write_executable(path: &Path, contents: &str) {
    use std::os::unix::fs::PermissionsExt;
    // Why: copying with cp avoids "Text file busy" when the script runs right after it is written.
    let source = path.with_extension("src");
    std::fs::write(&source, contents).expect("write the executable's source");
    let copied = massorca_process::command("cp")
        .arg(&source)
        .arg(path)
        .status()
        .expect("run cp");
    assert!(copied.success(), "cp {} failed", source.display());
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("make it executable");
}

#[cfg(not(unix))]
fn write_executable(path: &Path, contents: &str) {
    std::fs::write(path, contents).expect("write the executable");
}
