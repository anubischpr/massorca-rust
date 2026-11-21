//! A terminal whose wait failed is `unverifiable`, never dead (0033): loss of contact is
//! not proof of death. Every wait is bounded.

mod manager_support;

use manager_support::{Managed, is_gone, marker_command, script_shell, spec, wait_until, watch};
use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::{ExitReason, TerminalState};

#[tokio::test]
async fn an_unverifiable_terminal_stays_usable_until_a_reap_confirms_its_exit() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let manager = Managed::with(|config| config.max_exited = 1);
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let pid = info.pid.expect("a pid");
    let mut first = watch(&manager, &info.id, None).await;
    first.wait_for_output().await;

    manager.simulate_failed_wait(&info.id).expect("simulate");

    let state = |id: &str| manager.get(id).expect("still listed").state;
    assert_eq!(state(&info.id), TerminalState::Unverifiable);
    let listed = manager.list(Some("ws"));
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].state, TerminalState::Unverifiable);
    assert!(listed[0].exit.is_none());
    assert_eq!(manager.live_count(), 1);

    // Input, output and resize keep working; no attachment is told it exited.
    let (command, expected) = marker_command("still-there");
    manager.input(&info.id, &command).expect("input");
    first.wait_for_text(&expected).await;
    let resized = manager
        .resize(
            &info.id,
            PtySize {
                cols: 120,
                rows: 40,
            },
        )
        .expect("resize");
    assert_eq!(resized.state, TerminalState::Unverifiable);
    while first.resized.is_empty() {
        first.step().await;
    }
    let mut second = watch(&manager, &info.id, None).await;
    // Items arrive in order, so an `Exited` would come before this marker.
    let (command, expected) = marker_command("attached");
    manager.input(&info.id, &command).expect("input");
    first.wait_for_text(&expected).await;
    second.wait_for_text(&expected).await;
    assert!(first.exited.is_none() && !first.ended);
    assert!(second.exited.is_none() && !second.ended);

    // Retention forgets exited terminals only.
    let mut quick = spec("ws", dir.path());
    quick.shell = Some(script_shell("exit 3"));
    for _ in 0..2 {
        let done = manager.create(quick.clone()).expect("create");
        wait_until("a quick terminal to exit", || {
            manager
                .get(&done.id)
                .map_or(true, |info| info.state == TerminalState::Exited)
        });
    }
    assert_eq!(state(&info.id), TerminalState::Unverifiable);
    assert_eq!(manager.live_count(), 1);

    // Kill still ends the tree; only the reap that follows makes it exited.
    let killed = manager.kill(&info.id, false).expect("kill");
    wait_until("the shell to end", || is_gone(pid));
    assert_eq!(killed.state, TerminalState::Exited);
    assert_eq!(
        killed.exit.expect("an exit record").reason,
        ExitReason::Killed
    );
    assert_eq!(first.wait_for_exit().await.state, TerminalState::Exited);
    assert_eq!(manager.live_count(), 0);
}
