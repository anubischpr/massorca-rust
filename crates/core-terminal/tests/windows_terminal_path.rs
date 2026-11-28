//! A PowerShell terminal on Windows starts with the registry PATH merged with the service's own (decision 0032,
//! TERM-17). These start real ConPTY sessions; the PATH values are read, never written.
#![cfg(windows)]

mod manager_support;

use std::path::{Path, PathBuf};

use manager_support::{Managed, TIMEOUT, spec, watch};
use massorca_core_terminal::shell::EnvSnapshot;
use massorca_core_terminal::terminal_path::{PathSource, start_environment};
use massorca_core_terminal::{ExitWait, ShellChoice};

/// A PATH entry the service had when it started; the registry does not list it.
const SERVICE_ENTRY: &str = r"C:\massorca-term17-service-entry";

/// Prints whether the terminal's PATH holds System32 and the service entry, and nothing of the PATH itself.
const PATH_SCRIPT: &str = r"$p = @($env:PATH -split ';'); $sys = Join-Path $env:SystemRoot 'System32'; Write-Output ('TERM17PATH system32=' + ($p -contains $sys) + ' service=' + ($p -contains 'C:\massorca-term17-service-entry'))";

/// The service's environment as if it had started before the registry PATH was read: its own PATH only.
fn service_env() -> EnvSnapshot {
    EnvSnapshot::current().with_var("PATH", SERVICE_ENTRY)
}

fn windows_powershell() -> PathBuf {
    let system_root = std::env::var_os("SystemRoot").expect("SystemRoot is set");
    Path::new(&system_root)
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe")
}

fn powershell(script: &str, program: String) -> ShellChoice {
    ShellChoice {
        program,
        args: vec![
            "-NoLogo".to_owned(),
            "-NoProfile".to_owned(),
            "-Command".to_owned(),
            script.to_owned(),
        ],
    }
}

#[test]
fn the_start_environment_puts_the_registry_entries_before_the_service_entry() {
    let base = service_env();
    let (env, source) = start_environment(&base);
    assert_eq!(source, PathSource::Registry);
    let path = env.get("PATH").expect("the terminal has a PATH");
    assert!(
        path.ends_with(SERVICE_ENTRY),
        "the service entry comes last"
    );
    let system32 = Path::new(&std::env::var_os("SystemRoot").expect("SystemRoot is set"))
        .join("system32")
        .display()
        .to_string();
    assert!(
        path.split(';')
            .any(|entry| entry.eq_ignore_ascii_case(&system32)),
        "the registry's System32 entry, with its variable expanded, is on the PATH"
    );
}

#[test]
fn a_bare_program_name_is_found_on_the_registry_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let manager = Managed::with(|config| config.base_env = service_env());
    let mut create = spec("ws", dir.path());
    create.shell = Some(powershell("exit 0", "powershell.exe".to_owned()));

    let info = manager
        .create(create)
        .expect("powershell.exe is on the registry PATH, which the service PATH lacks");
    assert_eq!(info.path_source, PathSource::Registry);
    match manager
        .wait_for_exit(&info.id, TIMEOUT)
        .expect("wait for exit")
    {
        ExitWait::Exited(exited) => assert_eq!(exited.exit.and_then(|exit| exit.code), Some(0)),
        other => panic!("the terminal did not exit: {other:?}"),
    }
}

#[tokio::test]
async fn the_terminal_path_is_the_registry_path_with_the_service_entry_after_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let manager = Managed::with(|config| config.base_env = service_env());
    let mut create = spec("ws", dir.path());
    create.shell = Some(powershell(
        PATH_SCRIPT,
        windows_powershell().display().to_string(),
    ));
    let info = manager.create(create).expect("start powershell");

    let mut output = watch(&manager, &info.id, None).await;
    output
        .wait_for_text("TERM17PATH system32=True service=True")
        .await;
    let exited = output.wait_for_exit().await;
    assert_eq!(exited.exit.and_then(|exit| exit.code), Some(0));
}

#[test]
fn a_terminal_reports_the_registry_as_its_path_source() {
    let dir = tempfile::tempdir().expect("temp dir");
    let manager = Managed::with(|config| config.base_env = service_env());
    let mut create = spec("ws", dir.path());
    create.shell = Some(powershell(
        "exit 0",
        windows_powershell().display().to_string(),
    ));
    let info = manager.create(create).expect("start powershell");
    assert_eq!(info.path_source, PathSource::Registry);
}
