//! PowerShell terminals on Windows (decision 0030, TERM-17): the default shell starts at its prompt with no arguments,
//! so nothing suppresses the user's profile; a resize reaches it while it runs; its exit code is recorded. Real ConPTY
//! sessions; every wait is bounded.
#![cfg(windows)]

mod manager_support;

use manager_support::{Managed, TIMEOUT, spec, watch};
use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::shell::{EnvSnapshot, default_shell};
use massorca_core_terminal::{ExitWait, ShellChoice, TerminalState};

/// PowerShell's prompt starts with `PS `, printed once its startup, profile included, has finished.
const PROMPT: &str = "PS ";

/// No `-NoProfile`: the default shell runs as an interactive PowerShell does, so its profile loads.
#[tokio::test]
async fn the_default_shell_starts_at_its_prompt() {
    let dir = tempfile::tempdir().expect("temp dir");
    let manager = Managed::new();
    let mut create = spec("ws", dir.path());
    create.shell = None;
    let info = manager.create(create).expect("start the default shell");

    let mut output = watch(&manager, &info.id, None).await;
    output.wait_for_text(PROMPT).await;
    manager.input(&info.id, b"exit\r").expect("type exit");
    let exited = output.wait_for_exit().await;
    assert_eq!(exited.state, TerminalState::Exited);
    assert_eq!(exited.exit.and_then(|exit| exit.code), Some(0));
}

#[tokio::test]
async fn a_resize_reaches_powershell_while_it_runs() {
    let dir = tempfile::tempdir().expect("temp dir");
    let manager = Managed::new();
    let shell = default_shell(&EnvSnapshot::current()).expect("PowerShell is installed");
    let mut create = spec("ws", dir.path());
    create.shell = Some(ShellChoice {
        program: shell.display().to_string(),
        args: vec!["-NoLogo".to_owned(), "-NoProfile".to_owned()],
    });
    let info = manager.create(create).expect("start PowerShell");

    let mut output = watch(&manager, &info.id, None).await;
    output.wait_for_text(PROMPT).await;
    manager
        .resize(
            &info.id,
            PtySize {
                cols: 120,
                rows: 40,
            },
        )
        .expect("resize the terminal");
    manager
        .input(
            &info.id,
            b"Write-Output ('SIZE ' + [Console]::WindowWidth + ' ' + [Console]::WindowHeight)\r",
        )
        .expect("type the size command");
    output.wait_for_text("SIZE 120 40").await;
    manager.input(&info.id, b"exit\r").expect("type exit");
    let exited = output.wait_for_exit().await;
    assert_eq!(exited.exit.and_then(|exit| exit.code), Some(0));
}

#[test]
fn powershell_exit_code_is_recorded() {
    let dir = tempfile::tempdir().expect("temp dir");
    let manager = Managed::new();
    let shell = default_shell(&EnvSnapshot::current()).expect("PowerShell is installed");
    let mut create = spec("ws", dir.path());
    create.shell = Some(ShellChoice {
        program: shell.display().to_string(),
        args: vec![
            "-NoLogo".to_owned(),
            "-NoProfile".to_owned(),
            "-Command".to_owned(),
            "exit 3".to_owned(),
        ],
    });
    let info = manager.create(create).expect("start PowerShell");
    match manager
        .wait_for_exit(&info.id, TIMEOUT)
        .expect("wait for exit")
    {
        ExitWait::Exited(exited) => {
            assert_eq!(exited.exit.and_then(|exit| exit.code), Some(3));
        }
        other => panic!("the terminal did not exit: {other:?}"),
    }
}
