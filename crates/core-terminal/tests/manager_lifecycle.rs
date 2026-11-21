//! `TerminalManager` lifecycle against real shells (decisions 0031 to 0033).
//! Every wait is bounded, so a failure reports instead of hanging the suite.

mod manager_support;

use std::time::{Duration, Instant};

use manager_support::{Managed, SIZE, script_shell, spec, wait_until};
use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::{ExitReason, ShellChoice, TerminalError, TerminalState};

const GRACE: Duration = Duration::from_secs(5);

fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("a temporary directory")
}

#[tokio::test]
async fn create_is_live_with_a_pid_and_listed_under_its_workspace() {
    let dir = temp_dir();
    let manager = Managed::new();

    let info = manager.create(spec("ws-a", dir.path())).expect("create");

    assert_eq!(info.state, TerminalState::Live);
    assert!(info.pid.is_some(), "{info:?}");
    assert_eq!(info.workspace_id, "ws-a");
    assert_eq!((info.cols, info.rows), (SIZE.cols, SIZE.rows));
    assert!(info.exit.is_none());
    let program = if cfg!(windows) { "cmd.exe" } else { "sh" };
    assert!(
        info.shell.to_ascii_lowercase().ends_with(program),
        "{}",
        info.shell
    );
    assert!(info.created_at_ms > 0);
    let ids = |workspace| -> Vec<String> {
        manager
            .list(workspace)
            .into_iter()
            .map(|terminal| terminal.id)
            .collect()
    };
    assert_eq!(ids(Some("ws-a")), vec![info.id.clone()]);
    assert!(ids(Some("ws-b")).is_empty());
    assert_eq!(ids(None), vec![info.id.clone()]);
    assert_eq!(manager.get(&info.id).expect("get").id, info.id);
    assert_eq!(manager.live_count(), 1);
}

#[tokio::test]
async fn list_is_ordered_by_creation() {
    let dir = temp_dir();
    let manager = Managed::new();
    let first = manager.create(spec("ws", dir.path())).expect("create");
    let second = manager.create(spec("ws", dir.path())).expect("create");
    let third = manager.create(spec("other", dir.path())).expect("create");

    let ids: Vec<String> = manager.list(None).into_iter().map(|t| t.id).collect();
    assert_eq!(ids, vec![first.id, second.id, third.id]);
}

#[tokio::test]
async fn force_kill_ends_the_shell_at_once() {
    let dir = temp_dir();
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");

    let started = Instant::now();
    let killed = manager.kill(&info.id, true).expect("kill");

    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(killed.state, TerminalState::Exited);
    assert_eq!(
        killed.exit.expect("an exit record").reason,
        ExitReason::Killed
    );
    assert_eq!(manager.live_count(), 0);
}

#[tokio::test]
async fn a_repeated_request_id_returns_the_first_terminal() {
    let dir = temp_dir();
    let manager = Managed::new();
    let mut create = spec("ws", dir.path());
    create.request_id = Some("request-1".to_owned());

    let first = manager.create(create.clone()).expect("create");
    let again = manager.create(create.clone()).expect("create again");
    create.request_id = Some("request-2".to_owned());
    let other = manager.create(create).expect("create another");

    assert_eq!(again.id, first.id);
    assert_ne!(other.id, first.id);
    assert_eq!(manager.list(None).len(), 2);
}

#[tokio::test]
async fn invalid_requests_spawn_nothing() {
    let dir = temp_dir();
    let manager = Managed::new();
    for (cols, rows) in [(1, 24), (1001, 24), (80, 0), (80, 501)] {
        let mut create = spec("ws", dir.path());
        create.size = PtySize { cols, rows };
        assert!(
            matches!(manager.create(create), Err(TerminalError::InvalidSize)),
            "{cols}x{rows}"
        );
    }
    let missing = dir.path().join("missing");
    assert!(matches!(
        manager.create(spec("ws", &missing)),
        Err(TerminalError::CwdNotFound(path)) if path == missing
    ));
    let file = dir.path().join("file.txt");
    std::fs::write(&file, "x").expect("write a file");
    assert!(matches!(
        manager.create(spec("ws", &file)),
        Err(TerminalError::CwdNotDirectory(path)) if path == file
    ));
    let mut create = spec("ws", dir.path());
    create.shell = Some(ShellChoice {
        program: "massorca-no-such-shell".to_owned(),
        args: Vec::new(),
    });
    assert!(matches!(
        manager.create(create),
        Err(TerminalError::ShellNotFound(name)) if name == "massorca-no-such-shell"
    ));
    assert!(manager.list(None).is_empty());
    assert_eq!(manager.live_count(), 0);
}

#[tokio::test]
async fn the_live_limit_refuses_more_terminals() {
    let dir = temp_dir();
    let manager = Managed::with(|config| config.max_live = 1);
    let first = manager.create(spec("ws", dir.path())).expect("create");

    assert!(matches!(
        manager.create(spec("ws", dir.path())),
        Err(TerminalError::LimitReached)
    ));

    manager.kill(&first.id, true).expect("kill");
    manager
        .create(spec("ws", dir.path()))
        .expect("a slot frees once the first terminal exited");
}

#[tokio::test]
async fn input_is_bounded_and_needs_a_live_terminal() {
    let dir = temp_dir();
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");

    assert!(matches!(
        manager.input(&info.id, &vec![b'a'; 1024 * 1024 + 1]),
        Err(TerminalError::InputTooLarge)
    ));

    manager.kill(&info.id, true).expect("kill");
    assert!(matches!(
        manager.input(&info.id, b"echo late"),
        Err(TerminalError::Exited)
    ));
    assert!(matches!(
        manager.resize(&info.id, PtySize { cols: 80, rows: 24 }),
        Err(TerminalError::Exited)
    ));
}

#[tokio::test]
async fn shutdown_kills_every_terminal() {
    let dir = temp_dir();
    let manager = Managed::new();
    let first = manager.create(spec("ws", dir.path())).expect("create");
    let second = manager.create(spec("other", dir.path())).expect("create");

    let started = Instant::now();
    manager.shutdown();

    assert!(
        started.elapsed() < GRACE + Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    for id in [&first.id, &second.id] {
        let info = manager.get(id).expect("listed after shutdown");
        assert_eq!(info.state, TerminalState::Exited);
        assert_eq!(
            info.exit.expect("an exit record").reason,
            ExitReason::ServiceStopped
        );
    }
    assert_eq!(manager.live_count(), 0);
}

#[tokio::test]
async fn only_the_newest_exited_terminals_are_kept() {
    let dir = temp_dir();
    let manager = Managed::with(|config| config.max_exited = 1);
    let mut create = spec("ws", dir.path());
    create.shell = Some(script_shell("exit 3"));
    let first = manager.create(create.clone()).expect("create");
    wait_until("the first terminal to exit", || {
        manager
            .get(&first.id)
            .is_ok_and(|info| info.state == TerminalState::Exited)
    });
    let second = manager.create(create).expect("create");

    wait_until("the second terminal to exit", || {
        manager
            .get(&second.id)
            .is_ok_and(|info| info.state == TerminalState::Exited)
    });

    assert!(matches!(
        manager.get(&first.id),
        Err(TerminalError::NotFound)
    ));
    let listed: Vec<String> = manager.list(None).into_iter().map(|t| t.id).collect();
    assert_eq!(listed, vec![second.id.clone()]);
    let exit = manager.get(&second.id).expect("get").exit.expect("exit");
    assert_eq!((exit.code, exit.reason), (Some(3), ExitReason::Exited));
}
