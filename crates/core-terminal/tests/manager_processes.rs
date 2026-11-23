//! The processes of each live terminal, as host memory reads them (decision 0081), against real shells.

mod manager_support;

use manager_support::{Managed, script_shell, spec, wait_until};
use massorca_core_terminal::TerminalState;

/// A shell that starts a child and waits for it, so both run while the test looks.
fn shell_with_a_child() -> massorca_core_terminal::ShellChoice {
    if cfg!(windows) {
        script_shell("ping -n 120 127.0.0.1 >nul")
    } else {
        // `; true` keeps `sh` from replacing itself with `sleep`.
        script_shell("sleep 120; true")
    }
}

#[test]
fn a_terminal_running_a_child_lists_the_shell_and_the_child() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let manager = Managed::new();
    let mut create = spec("ws-memory", dir.path());
    create.shell = Some(shell_with_a_child());
    let info = manager.create(create).expect("create");
    let shell_pid = info.pid.expect("the shell has a pid");

    let mut listed = Vec::new();
    wait_until("the shell and its child are listed", || {
        listed = manager.processes();
        listed
            .iter()
            .any(|terminal| terminal.terminal_id == info.id && terminal.pids.len() >= 2)
    });
    let terminal = listed
        .iter()
        .find(|terminal| terminal.terminal_id == info.id)
        .expect("the terminal is listed");
    assert_eq!(terminal.workspace_id, "ws-memory");
    assert!(
        terminal.pids.contains(&shell_pid),
        "{shell_pid} not in {:?}",
        terminal.pids
    );

    let killed = manager.kill(&info.id, true).expect("kill");
    assert_eq!(killed.state, TerminalState::Exited);
    assert!(
        manager
            .processes()
            .iter()
            .all(|terminal| terminal.terminal_id != info.id),
        "an exited terminal has no processes to read"
    );
}
