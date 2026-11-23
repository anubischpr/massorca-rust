//! The member pids of each live terminal, as listening ports read them (decision 0081), against real shells.

mod manager_support;

use manager_support::{Managed, script_shell, spec, wait_until};
use massorca_core_terminal::ShellChoice;
use massorca_host_stats::processes;

/// A shell that starts a child and waits for it, so both run while the test looks.
fn shell_with_a_child() -> ShellChoice {
    if cfg!(windows) {
        script_shell("ping -n 120 127.0.0.1 >nul")
    } else {
        // `; true` keeps `sh` from replacing itself with `sleep`.
        script_shell("sleep 120; true")
    }
}

#[test]
fn a_terminal_lists_its_shell_and_child_and_never_the_service() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let manager = Managed::new();
    let mut create = spec("ws-members", dir.path());
    create.shell = Some(shell_with_a_child());
    let info = manager.create(create).expect("create");
    let shell_pid = info.pid.expect("the shell has a pid");

    let mut listed = Vec::new();
    wait_until("the shell and its child are members", || {
        listed = manager.member_pids(&processes());
        listed.iter().any(|terminal| {
            terminal.terminal_id == info.id
                && terminal.pids.as_ref().is_some_and(|pids| pids.len() >= 2)
        })
    });
    let terminal = listed
        .iter()
        .find(|terminal| terminal.terminal_id == info.id)
        .expect("the terminal is listed");
    assert_eq!(terminal.workspace_id, "ws-members");
    let pids = terminal.pids.as_ref().expect("the members are listed");
    assert!(pids.contains(&shell_pid), "{pids:?}");
    assert!(
        !pids.contains(&std::process::id()),
        "the service is never a member: {pids:?}"
    );

    manager.kill(&info.id, true).expect("kill");
}

#[cfg(windows)]
#[test]
fn still_members_keeps_only_the_pids_that_are_in_the_terminals_job() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let manager = Managed::new();
    let mut create = spec("ws-members", dir.path());
    create.shell = Some(shell_with_a_child());
    let info = manager.create(create).expect("create");
    let shell_pid = info.pid.expect("the shell has a pid");

    let kept = manager.still_members(&info.id, &[shell_pid, std::process::id()]);
    assert_eq!(kept, [shell_pid], "the service is in no terminal's job");

    manager.kill(&info.id, true).expect("kill");
    assert!(
        manager.still_members(&info.id, &[shell_pid]).is_empty(),
        "an ended terminal keeps no member"
    );
}
