//! `TerminalManager::wait_for_exit`: the exit code of a script, a terminal that already exited, a timeout that leaves a
//! live terminal live, a terminal closed during the wait, an unverifiable terminal, and the cap on concurrent waits.
//! Every wait is bounded.

mod manager_support;

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use manager_support::{Managed, TIMEOUT, script_shell, spec, wait_until};
use massorca_core_terminal::{CreateSpec, ExitWait, TerminalState};

/// A spec for a shell that runs `script` and exits, in workspace `ws`.
fn script(dir: &Path, script: &str) -> CreateSpec {
    let mut quick = spec("ws", dir);
    quick.shell = Some(script_shell(script));
    quick
}

#[test]
fn a_wait_reports_the_exit_code_of_a_script_that_ends() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let manager = Managed::new();
    let info = manager
        .create(script(dir.path(), "exit 3"))
        .expect("create");

    match manager
        .wait_for_exit(&info.id, TIMEOUT)
        .expect("the wait runs")
    {
        ExitWait::Exited(exited) => {
            assert_eq!(exited.state, TerminalState::Exited);
            assert_eq!(exited.exit.expect("an exit record").code, Some(3));
        }
        other => panic!("expected the exit, got {other:?}"),
    }
}

#[test]
fn a_wait_on_a_terminal_that_already_exited_answers_at_once() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let manager = Managed::new();
    let info = manager
        .create(script(dir.path(), "exit 3"))
        .expect("create");
    wait_until("the script to exit", || {
        manager
            .get(&info.id)
            .is_ok_and(|now| now.state == TerminalState::Exited)
    });

    let started = Instant::now();
    let outcome = manager
        .wait_for_exit(&info.id, TIMEOUT)
        .expect("the wait runs");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "an exited terminal does not wait: {:?}",
        started.elapsed()
    );
    match outcome {
        ExitWait::Exited(exited) => {
            assert_eq!(exited.exit.expect("an exit record").code, Some(3));
        }
        other => panic!("expected the exit, got {other:?}"),
    }
}

#[test]
fn a_wait_that_runs_out_leaves_a_live_terminal_live() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");

    let started = Instant::now();
    let outcome = manager
        .wait_for_exit(&info.id, Duration::from_millis(300))
        .expect("the wait runs");
    assert!(
        started.elapsed() >= Duration::from_millis(300),
        "the wait ends at its deadline, not before"
    );
    match outcome {
        ExitWait::TimedOut(live) => {
            assert_eq!(live.state, TerminalState::Live);
            assert!(live.exit.is_none());
        }
        other => panic!("expected the timeout, got {other:?}"),
    }
    manager.kill(&info.id, true).expect("kill");
}

#[test]
fn a_terminal_closed_during_a_wait_is_gone_not_exited() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let waiter = {
        let manager = manager.0.clone();
        let id = info.id.clone();
        thread::spawn(move || manager.wait_for_exit(&id, TIMEOUT))
    };
    // A wait that starts after the close finds no terminal at all, so the waiter gets a head start.
    thread::sleep(Duration::from_millis(300));

    manager.close(&info.id).expect("close");

    let outcome = waiter
        .join()
        .expect("the waiter ends")
        .expect("the wait runs");
    assert_eq!(outcome, ExitWait::Gone);
}

#[test]
fn an_unverifiable_terminal_times_out_and_is_never_reported_exited() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");
    manager.simulate_failed_wait(&info.id).expect("simulate");

    match manager
        .wait_for_exit(&info.id, Duration::from_millis(300))
        .expect("the wait runs")
    {
        ExitWait::TimedOut(state) => {
            assert_eq!(state.state, TerminalState::Unverifiable);
            assert!(state.exit.is_none());
        }
        other => panic!("expected the timeout, got {other:?}"),
    }
    manager.kill(&info.id, true).expect("kill");
}

#[test]
fn a_wait_past_the_cap_on_concurrent_waits_is_refused() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let manager = Managed::with(|config| config.max_waits = 1);
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let holder = {
        let manager = manager.0.clone();
        let id = info.id.clone();
        thread::spawn(move || manager.wait_for_exit(&id, TIMEOUT))
    };
    thread::sleep(Duration::from_millis(300));

    let refused = manager
        .wait_for_exit(&info.id, Duration::from_millis(10))
        .expect("the refusal is an outcome, not a failure of the terminal");
    assert_eq!(refused, ExitWait::TooManyWaits);

    manager.kill(&info.id, true).expect("kill");
    let held = holder
        .join()
        .expect("the holder ends")
        .expect("the holder's wait runs");
    assert!(matches!(held, ExitWait::Exited(_)), "got {held:?}");
}
