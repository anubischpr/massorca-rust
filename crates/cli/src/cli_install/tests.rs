//! Tests of the install, uninstall, status and text operations.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::tempdir;

use super::fake_user_path::FakeUserPath;
use super::*;

fn windows_env(root: &Path, store: &FakeUserPath) -> Environment {
    Environment::Windows {
        layout: Layout::windows(root),
        target: PathBuf::from("/opt/massorca/massorca.exe"),
        store: Box::new(store.clone()),
        variables: Vec::new(),
    }
}

/// A Windows environment that runs `target`, so tests can stand in for a binary that an update moved.
fn windows_env_for(root: &Path, store: &FakeUserPath, target: &Path) -> Environment {
    Environment::Windows {
        layout: Layout::windows(root),
        target: target.to_path_buf(),
        store: Box::new(store.clone()),
        variables: Vec::new(),
    }
}

/// Creates a file standing in for a binary, with its folder.
fn write_binary(path: &Path) {
    fs::create_dir_all(path.parent().expect("a folder")).expect("folder");
    fs::write(path, "").expect("binary");
}

#[cfg(unix)]
fn write_program(path: &Path, name: &str) {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(path.parent().expect("a folder")).expect("folder");
    fs::write(path, format!("#!/bin/sh\necho {name}\n")).expect("program");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("executable");
}

#[test]
fn windows_install_writes_the_command_file_and_adds_its_folder_to_the_user_path_once() {
    let root = tempdir().expect("temp dir");
    let store = FakeUserPath::with_text(r"C:\Windows");
    let env = windows_env(root.path(), &store);
    let first = install(&env).expect("install");
    assert_eq!(first.state, State::Installed);
    assert!(first.changed);
    assert_eq!(first.bin_dir_on_path, Some(true));
    assert_eq!(
        first.points_to.as_deref(),
        Some("/opt/massorca/massorca.exe")
    );
    assert_eq!(store.writes(), 1);
    let second = install(&env).expect("second install");
    assert!(!second.changed);
    assert_eq!(store.writes(), 1);
}

#[test]
fn windows_install_stops_before_any_change_when_the_user_path_cannot_be_read() {
    let root = tempdir().expect("temp dir");
    let env = windows_env(root.path(), &FakeUserPath::unreadable());
    let error = install(&env).expect_err("unreadable PATH");
    assert_eq!(error.code, "cli.user_path_unreadable");
    assert!(!Layout::windows(root.path()).command_path.exists());
}

#[test]
fn windows_install_refuses_a_foreign_command_file_before_touching_the_user_path() {
    let root = tempdir().expect("temp dir");
    let layout = Layout::windows(root.path());
    fs::create_dir_all(&layout.bin_dir).expect("folder");
    fs::write(&layout.command_path, "@echo off\r\ncall other.bat\r\n").expect("foreign");
    let store = FakeUserPath::with_text(r"C:\Windows");
    let error = install(&windows_env(root.path(), &store)).expect_err("refused");
    assert_eq!(error.code, "cli.command_conflict");
    assert_eq!(store.writes(), 0);
}

#[test]
fn install_refuses_a_target_that_cmd_would_misread_before_anything_is_written() {
    let root = tempdir().expect("temp dir");
    let store = FakeUserPath::with_text(r"C:\Windows");
    let env = Environment::Windows {
        layout: Layout::windows(root.path()),
        target: PathBuf::from("/opt/a%b/massorca.exe"),
        store: Box::new(store.clone()),
        variables: Vec::new(),
    };
    assert_eq!(
        install(&env).expect_err("refused").code,
        "cli.invalid_target"
    );
    assert!(!Layout::windows(root.path()).command_path.exists());
    assert_eq!(store.writes(), 0);
}

#[test]
fn windows_uninstall_removes_the_command_and_its_path_entry_once_the_folder_is_empty() {
    let root = tempdir().expect("temp dir");
    let store = FakeUserPath::with_text(r"C:\Windows");
    let env = windows_env(root.path(), &store);
    install(&env).expect("install");
    let removed = uninstall(&env).expect("uninstall");
    assert!(removed.changed);
    assert_eq!(removed.state, State::NotInstalled);
    assert!(!Layout::windows(root.path()).bin_dir.exists());
    assert_eq!(store.text(), r"C:\Windows");
}

#[test]
fn windows_uninstall_keeps_the_path_entry_while_the_folder_holds_other_files() {
    let root = tempdir().expect("temp dir");
    let store = FakeUserPath::with_text(r"C:\Windows");
    let env = windows_env(root.path(), &store);
    install(&env).expect("install");
    fs::write(Layout::windows(root.path()).bin_dir.join("other.cmd"), "").expect("other file");
    let writes = store.writes();
    let report = uninstall(&env).expect("uninstall");
    assert_eq!(report.state, State::NotInstalled);
    assert_eq!(report.bin_dir_on_path, Some(true));
    assert_eq!(store.writes(), writes);
}

#[test]
fn windows_uninstall_refuses_a_foreign_command_file_and_leaves_the_user_path_alone() {
    let root = tempdir().expect("temp dir");
    let layout = Layout::windows(root.path());
    fs::create_dir_all(&layout.bin_dir).expect("folder");
    fs::write(&layout.command_path, "@echo off\r\ncall other.bat\r\n").expect("foreign");
    let store = FakeUserPath::with_text(r"C:\Windows");
    let error = uninstall(&windows_env(root.path(), &store)).expect_err("refused");
    assert_eq!(error.code, "cli.command_conflict");
    assert!(layout.command_path.exists());
    assert_eq!(store.writes(), 0);
}

#[test]
fn windows_status_reports_not_on_path_then_a_different_binary() {
    let root = tempdir().expect("temp dir");
    let first = root.path().join("first").join("massorca.exe");
    let other = root.path().join("other").join("massorca.exe");
    write_binary(&first);
    write_binary(&other);
    let store = FakeUserPath::with_text(r"C:\Windows");
    let env = windows_env_for(root.path(), &store, &first);
    install(&env).expect("install");
    store.set_text(r"C:\Windows");
    assert_eq!(status(&env).expect("status").state, State::NotOnPath);
    let report = status(&windows_env_for(root.path(), &store, &other)).expect("status");
    assert_eq!(report.state, State::DifferentBinary);
    assert_eq!(
        report.points_to.as_deref(),
        Some(first.to_str().expect("a UTF-8 path"))
    );
}

#[test]
fn a_command_whose_binary_an_update_moved_is_stale_until_repair_points_it_at_the_new_binary() {
    let root = tempdir().expect("temp dir");
    let old = root.path().join("old").join("massorca.exe");
    let new = root.path().join("new").join("massorca.exe");
    write_binary(&old);
    let store = FakeUserPath::with_text(r"C:\Windows");
    install(&windows_env_for(root.path(), &store, &old)).expect("install");
    // The update moves the binary: the old copy goes and the new one is in place.
    fs::remove_dir_all(old.parent().expect("a folder")).expect("old folder");
    write_binary(&new);
    let current = windows_env_for(root.path(), &store, &new);
    assert_eq!(status(&current).expect("status").state, State::Stale);
    assert!(repair_gone(&current).expect("repair"));
    assert_eq!(status(&current).expect("status").state, State::Installed);
    assert!(!repair_gone(&current).expect("second repair"));
}

#[test]
fn repair_leaves_a_command_for_a_binary_that_is_still_there_alone() {
    let root = tempdir().expect("temp dir");
    let first = root.path().join("first").join("massorca.exe");
    let other = root.path().join("other").join("massorca.exe");
    write_binary(&first);
    write_binary(&other);
    let store = FakeUserPath::with_text(r"C:\Windows");
    install(&windows_env_for(root.path(), &store, &first)).expect("install");
    let current = windows_env_for(root.path(), &store, &other);
    assert!(!repair_gone(&current).expect("repair"));
    assert_eq!(
        status(&current).expect("status").state,
        State::DifferentBinary
    );
}

#[test]
fn repair_leaves_a_foreign_command_file_alone() {
    let root = tempdir().expect("temp dir");
    let layout = Layout::windows(root.path());
    fs::create_dir_all(&layout.bin_dir).expect("folder");
    fs::write(&layout.command_path, "@echo off\r\ncall other.bat\r\n").expect("foreign");
    let store = FakeUserPath::with_text(r"C:\Windows");
    let env = windows_env_for(root.path(), &store, &root.path().join("massorca.exe"));
    assert!(!repair_gone(&env).expect("repair"));
    assert_eq!(
        fs::read_to_string(&layout.command_path).expect("foreign"),
        "@echo off\r\ncall other.bat\r\n"
    );
}

#[test]
fn uninstall_removes_a_stale_command_its_folder_and_its_path_entry() {
    let root = tempdir().expect("temp dir");
    let old = root.path().join("old").join("massorca.exe");
    let new = root.path().join("new").join("massorca.exe");
    write_binary(&old);
    let store = FakeUserPath::with_text(r"C:\Windows");
    install(&windows_env_for(root.path(), &store, &old)).expect("install");
    fs::remove_dir_all(old.parent().expect("a folder")).expect("old folder");
    write_binary(&new);
    let current = windows_env_for(root.path(), &store, &new);
    assert_eq!(status(&current).expect("status").state, State::Stale);
    let removed = uninstall(&current).expect("uninstall");
    assert!(removed.changed);
    assert_eq!(removed.state, State::NotInstalled);
    assert!(!Layout::windows(root.path()).bin_dir.exists());
    assert_eq!(store.text(), r"C:\Windows");
}

#[test]
fn the_text_for_a_stale_command_names_the_missing_binary_and_the_fix() {
    let report = Report {
        state: State::Stale,
        method: "symlink",
        command: "/home/ana/.local/bin/massorca".to_owned(),
        target: "/opt/new/massorca".to_owned(),
        points_to: Some("/opt/old/massorca".to_owned()),
        bin_dir: "/home/ana/.local/bin".to_owned(),
        bin_dir_on_path: Some(true),
        changed: false,
    };
    let lines = lines(Operation::Status, &report, Platform::Unix);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("/opt/old/massorca") && line.contains("no longer there")),
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("massorca cli install")),
        "{lines:?}"
    );
}

#[cfg(unix)]
#[test]
fn the_installed_command_runs_the_binary_an_update_put_in_place_of_the_moved_one() {
    let root = tempdir().expect("temp dir");
    let layout = Layout::unix(root.path());
    let old = root.path().join("old").join("massorca");
    let new = root.path().join("new").join("massorca");
    write_program(&old, "old");
    let path = format!("/usr/bin:{}", layout.bin_dir.display());
    let installed = Environment::Unix {
        layout: layout.clone(),
        target: old.clone(),
        path: path.clone(),
    };
    assert_eq!(
        install(&installed).expect("install").state,
        State::Installed
    );
    fs::remove_dir_all(old.parent().expect("a folder")).expect("old folder");
    write_program(&new, "new");
    let current = Environment::Unix {
        layout: layout.clone(),
        target: new,
        path,
    };
    assert_eq!(status(&current).expect("status").state, State::Stale);
    assert!(repair_gone(&current).expect("repair"));
    assert_eq!(status(&current).expect("status").state, State::Installed);
    let output = massorca_process::command(&layout.command_path)
        .output()
        .expect("the command runs");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "new\n");
}

#[test]
fn status_says_nothing_about_the_path_when_the_user_path_cannot_be_read() {
    let root = tempdir().expect("temp dir");
    let env = windows_env(root.path(), &FakeUserPath::unreadable());
    let report = status(&env).expect("status");
    assert_eq!(report.state, State::NotInstalled);
    assert_eq!(report.bin_dir_on_path, None);
}

#[test]
fn status_on_unix_reports_not_installed_and_the_folder_off_path_when_nothing_is_there() {
    let root = tempdir().expect("temp dir");
    let env = Environment::Unix {
        layout: Layout::unix(root.path()),
        target: PathBuf::from("/opt/massorca/massorca"),
        path: "/usr/bin:/bin".to_owned(),
    };
    let report = status(&env).expect("status");
    assert_eq!(report.state, State::NotInstalled);
    assert_eq!(report.bin_dir_on_path, Some(false));
    assert_eq!(report.points_to, None);
    assert!(!report.changed);
}

#[cfg(unix)]
#[test]
fn unix_install_links_the_command_once_and_says_when_its_folder_is_off_path() {
    let root = tempdir().expect("temp dir");
    let env = Environment::Unix {
        layout: Layout::unix(root.path()),
        target: PathBuf::from("/opt/massorca/massorca"),
        path: "/usr/bin".to_owned(),
    };
    let first = install(&env).expect("install");
    assert!(first.changed);
    assert_eq!(first.state, State::NotOnPath);
    assert_eq!(first.points_to.as_deref(), Some("/opt/massorca/massorca"));
    assert!(!install(&env).expect("second install").changed);
}

#[cfg(unix)]
#[test]
fn unix_install_reports_installed_on_path_and_uninstall_removes_only_the_link() {
    let root = tempdir().expect("temp dir");
    let layout = Layout::unix(root.path());
    let env = Environment::Unix {
        layout: layout.clone(),
        target: PathBuf::from("/opt/massorca/massorca"),
        path: format!("/usr/bin:{}", layout.bin_dir.display()),
    };
    assert_eq!(install(&env).expect("install").state, State::Installed);
    let removed = uninstall(&env).expect("uninstall");
    assert!(removed.changed);
    assert_eq!(removed.state, State::NotInstalled);
    assert!(layout.bin_dir.exists());
    assert!(!uninstall(&env).expect("second uninstall").changed);
}

#[cfg(unix)]
#[test]
fn unix_install_refuses_a_regular_file_at_the_command_path() {
    let root = tempdir().expect("temp dir");
    let layout = Layout::unix(root.path());
    fs::create_dir_all(&layout.bin_dir).expect("folder");
    fs::write(&layout.command_path, "#!/bin/sh\n").expect("file");
    let env = Environment::Unix {
        layout: layout.clone(),
        target: PathBuf::from("/opt/massorca/massorca"),
        path: String::new(),
    };
    assert_eq!(
        install(&env).expect_err("refused").code,
        "cli.command_conflict"
    );
    assert!(layout.command_path.exists());
}

#[test]
fn the_text_for_a_command_off_the_path_says_where_to_add_the_folder() {
    let report = Report {
        state: State::NotOnPath,
        method: "symlink",
        command: "/home/ana/.local/bin/massorca".to_owned(),
        target: "/opt/massorca/massorca".to_owned(),
        points_to: Some("/opt/massorca/massorca".to_owned()),
        bin_dir: "/home/ana/.local/bin".to_owned(),
        bin_dir_on_path: Some(false),
        changed: false,
    };
    let lines = lines(Operation::Status, &report, Platform::Unix);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("/home/ana/.local/bin is not on your PATH")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|line| line.contains("shell profile")),
        "{lines:?}"
    );
}
