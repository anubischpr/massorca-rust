//! `TerminalManager` lifecycle against real shells (decisions 0031 to 0033).
//! Every wait is bounded, so a failure reports instead of hanging the suite.

mod manager_support;

use std::thread;
use std::time::{Duration, Instant};

use manager_support::{
    Managed, SIZE, TIMEOUT, Watch, is_gone, line, marker_command, script_shell, spec, visible,
    wait_until, watch,
};
use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::{
    AttachItem, AttachMode, ExitReason, ShellChoice, TerminalError, TerminalState,
};

/// Far above any load-induced delay, so finishing under it proves the grace was not waited out.
const GRACE: Duration = Duration::from_secs(60);
/// Upper bound for work that must not wait the grace.
const PROMPT: Duration = Duration::from_secs(30);

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
async fn input_reaches_the_shell() {
    let dir = temp_dir();
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let mut watch = watch(&manager, &info.id, None).await;

    let (command, expected) = marker_command("marker-1");
    manager.input(&info.id, &command).expect("input");

    watch.wait_for_text(&expected).await;
}

#[tokio::test]
async fn resize_updates_the_info_and_the_screen() {
    let dir = temp_dir();
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let mut first = watch(&manager, &info.id, None).await;

    let size = PtySize {
        cols: 100,
        rows: 30,
    };
    let resized = manager.resize(&info.id, size).expect("resize");

    assert_eq!((resized.cols, resized.rows), (100, 30));
    let shown = manager.get(&info.id).expect("get");
    assert_eq!((shown.cols, shown.rows), (100, 30));
    while first.resized.is_empty() {
        first.step().await;
    }
    assert_eq!(first.resized, vec![(100, 30)]);
    let second = watch(&manager, &info.id, None).await;
    assert_eq!(second.snapshot_size, (100, 30));
}

#[tokio::test]
async fn output_produced_after_a_resize_follows_its_resized_item() {
    let dir = temp_dir();
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let mut watch = watch(&manager, &info.id, None).await;
    watch.wait_for_output().await;

    manager
        .resize(
            &info.id,
            PtySize {
                cols: 100,
                rows: 30,
            },
        )
        .expect("resize");
    let (command, expected) = marker_command("after-resize");
    manager.input(&info.id, &command).expect("input");

    let deadline = Instant::now() + TIMEOUT;
    while !watch.visible.contains(&expected) {
        assert!(
            !watch.ended && Instant::now() < deadline,
            "no output after the resize: {:?}",
            watch.visible.tail()
        );
        watch.step().await;
        if watch.visible.contains(&expected) {
            // That output was produced after `resize` returned, so its `Resized` came first.
            assert_eq!(watch.resized, vec![(100, 30)]);
        }
    }
}

#[tokio::test]
async fn a_shell_that_exits_by_itself_is_exited_with_its_code() {
    let dir = temp_dir();
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let mut watch = watch(&manager, &info.id, None).await;
    watch.wait_for_output().await;

    manager.input(&info.id, &line("exit 7")).expect("input");

    let exited = watch.wait_for_exit().await;
    assert_eq!(exited.state, TerminalState::Exited);
    let exit = exited.exit.expect("an exit record");
    assert_eq!(exit.code, Some(7));
    assert_eq!(exit.reason, ExitReason::Exited);
    assert!(exit.at_ms >= info.created_at_ms);
    assert_eq!(
        manager.get(&info.id).expect("get").state,
        TerminalState::Exited
    );
    assert_eq!(manager.live_count(), 0);
}

#[tokio::test]
async fn graceful_kill_ends_an_interactive_shell_within_the_grace() {
    let dir = temp_dir();
    let manager = Managed::with(|config| config.kill_grace = GRACE);
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let mut watch = watch(&manager, &info.id, None).await;
    watch.wait_for_output().await;

    let started = Instant::now();
    let killed = manager.kill(&info.id, false).expect("kill");

    assert!(started.elapsed() < PROMPT, "{:?}", started.elapsed());
    assert_eq!(killed.state, TerminalState::Exited);
    assert_eq!(
        killed.exit.expect("an exit record").reason,
        ExitReason::Killed
    );
    let seen = watch.wait_for_exit().await;
    assert_eq!(seen.state, TerminalState::Exited);
}

#[tokio::test]
async fn force_kill_ends_the_shell_at_once() {
    let dir = temp_dir();
    let manager = Managed::with(|config| config.kill_grace = GRACE);
    let info = manager.create(spec("ws", dir.path())).expect("create");

    let started = Instant::now();
    let killed = manager.kill(&info.id, true).expect("kill");

    assert!(started.elapsed() < PROMPT, "{:?}", started.elapsed());
    assert_eq!(killed.state, TerminalState::Exited);
    assert_eq!(
        killed.exit.expect("an exit record").reason,
        ExitReason::Killed
    );
    // Read at once: the kill returns only after the terminal stopped counting as live.
    assert_eq!(
        manager.live_count(),
        0,
        "still counted live when kill returned"
    );
}

/// The pid a `child=<pid>` line names.
fn child_pid(text: &str) -> Option<u32> {
    let start = text.find("child=")? + "child=".len();
    let digits: String = text[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    // The line is complete once something follows the digits.
    if text[start + digits.len()..].is_empty() {
        return None;
    }
    digits.parse().ok()
}

/// A shell that starts a background process, prints `child=<pid>;`, then runs `then`.
fn background_shell(then: &str) -> ShellChoice {
    if cfg!(windows) {
        let script = format!(
            "$p = Start-Process -FilePath ping.exe -ArgumentList '-n','120','127.0.0.1' \
             -NoNewWindow -PassThru; Write-Output ('child=' + $p.Id + ';'); {then}"
        );
        ShellChoice {
            program: "powershell.exe".to_owned(),
            args: vec![
                "-NoLogo".to_owned(),
                "-NoProfile".to_owned(),
                "-Command".to_owned(),
                script,
            ],
        }
    } else {
        ShellChoice {
            program: "bash".to_owned(),
            args: vec![
                "-c".to_owned(),
                format!("set -m; sleep 120 & echo \"child=$!;\"; {then}"),
            ],
        }
    }
}

/// Reads the pid that `background_shell` prints.
async fn read_child_pid(watch: &mut Watch) -> u32 {
    loop {
        if let Some(pid) = child_pid(&watch.visible.as_string()) {
            return pid;
        }
        assert!(
            !watch.ended,
            "the shell ended before printing its child: {:?}",
            watch.visible.tail()
        );
        watch.step().await;
    }
}

#[tokio::test]
async fn kill_ends_a_background_grandchild() {
    let dir = temp_dir();
    let manager = Managed::new();
    let mut create = spec("ws", dir.path());
    create.shell = Some(background_shell("sleep 120"));
    let info = manager.create(create).expect("create");
    let mut watch = watch(&manager, &info.id, None).await;
    let pid = read_child_pid(&mut watch).await;
    assert!(!is_gone(pid), "the grandchild {pid} runs before the kill");

    manager.kill(&info.id, false).expect("kill");

    wait_until("the grandchild to end", || is_gone(pid));
}

#[tokio::test]
async fn kill_after_the_shell_exited_ends_what_it_left_running() {
    let dir = temp_dir();
    let manager = Managed::new();
    let mut create = spec("ws", dir.path());
    create.shell = Some(background_shell("exit 0"));
    let info = manager.create(create).expect("create");
    let mut watch = watch(&manager, &info.id, None).await;
    let pid = read_child_pid(&mut watch).await;
    let exited = watch.wait_for_exit().await;
    assert_eq!(exited.state, TerminalState::Exited);

    let killed = manager
        .kill(&info.id, false)
        .expect("kill an exited terminal");

    assert_eq!(killed.state, TerminalState::Exited);
    wait_until("the grandchild to end", || is_gone(pid));
}

#[tokio::test]
async fn a_shell_that_exits_by_itself_ends_what_it_left_running() {
    let dir = temp_dir();
    let manager = Managed::new();
    let mut create = spec("ws", dir.path());
    create.shell = Some(background_shell("exit 0"));
    let info = manager.create(create).expect("create");
    let mut watch = watch(&manager, &info.id, None).await;
    let pid = read_child_pid(&mut watch).await;

    let exited = watch.wait_for_exit().await;

    assert_eq!(exited.state, TerminalState::Exited);
    let exit = exited.exit.expect("an exit record");
    assert_eq!(exit.reason, ExitReason::Exited);
    // On Unix the leftovers end before the exit is recorded, so they are gone by now.
    if cfg!(unix) {
        assert!(is_gone(pid), "the grandchild {pid} outlived its shell");
    }
    wait_until("the grandchild to end", || is_gone(pid));
}

#[tokio::test]
async fn close_kills_and_forgets() {
    let dir = temp_dir();
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");

    let closed = manager.close(&info.id).expect("close");

    assert_eq!(closed.state, TerminalState::Exited);
    assert_eq!(
        closed.exit.expect("an exit record").reason,
        ExitReason::Killed
    );
    assert!(matches!(
        manager.get(&info.id),
        Err(TerminalError::NotFound)
    ));
    assert!(manager.list(None).is_empty());
    assert!(matches!(
        manager.input(&info.id, b"x"),
        Err(TerminalError::NotFound)
    ));
    assert!(matches!(
        manager.kill(&info.id, true),
        Err(TerminalError::NotFound)
    ));
    assert!(matches!(
        manager.close(&info.id),
        Err(TerminalError::NotFound)
    ));
    assert!(matches!(
        manager.attach(&info.id, None),
        Err(TerminalError::NotFound)
    ));
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
    // Shutdown uses its own short grace, never the kill grace.
    let manager = Managed::with(|config| config.kill_grace = GRACE);
    let first = manager.create(spec("ws", dir.path())).expect("create");
    let second = manager.create(spec("other", dir.path())).expect("create");

    let started = Instant::now();
    manager.shutdown();

    assert!(started.elapsed() < PROMPT, "{:?}", started.elapsed());
    for id in [&first.id, &second.id] {
        // Shutdown is bounded; a slow drain records the exit just after it returns.
        wait_until("the exit after shutdown", || {
            manager
                .get(id)
                .is_ok_and(|info| info.state == TerminalState::Exited)
        });
        let info = manager.get(id).expect("listed after shutdown");
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

#[tokio::test]
async fn a_terminal_without_attachments_still_renders_output() {
    let dir = temp_dir();
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let (command, expected) = marker_command("unattended");
    manager.input(&info.id, &command).expect("input");
    // ConPTY asks for the cursor position at start; only the manager can answer it here.
    thread::sleep(Duration::from_millis(500));

    let deadline = Instant::now() + TIMEOUT;
    loop {
        let mut attachment = manager.attach(&info.id, None).expect("attach");
        let mut seen = String::new();
        for _ in 0..2 {
            match tokio::time::timeout(TIMEOUT, attachment.next()).await {
                Ok(Some(AttachItem::Attached { mode, .. })) => {
                    assert_eq!(mode, AttachMode::Snapshot);
                }
                Ok(Some(AttachItem::Snapshot { bytes, .. })) => {
                    seen = visible(&bytes).as_string();
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        drop(attachment);
        if seen.contains(&expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "no output without attachments: {seen:?}"
        );
        thread::sleep(Duration::from_millis(200));
    }
}

/// Polls one-off snapshots, each a client that never answers a cursor query, until the
/// screen shows `expected`.
async fn wait_for_screen(manager: &Managed, id: &str, expected: &str) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let mut attachment = manager.attach(id, None).expect("attach");
        let mut seen = String::new();
        for _ in 0..2 {
            if let Ok(Some(AttachItem::Snapshot { bytes, .. })) =
                tokio::time::timeout(TIMEOUT, attachment.next()).await
            {
                seen = visible(&bytes).as_string();
            }
        }
        drop(attachment);
        if seen.contains(expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the screen never showed {expected:?}: {seen:?}"
        );
        thread::sleep(Duration::from_millis(200));
    }
}

#[tokio::test]
async fn a_cursor_query_seen_by_a_silent_client_is_answered_when_it_detaches() {
    let dir = temp_dir();
    let manager = Managed::new();
    let mut create = spec("ws", dir.path());
    let expected = if cfg!(windows) {
        // ConPTY asks at start and renders nothing until it is answered.
        String::new()
    } else {
        // Asks only once the test is attached, then waits for the reply (`ESC [ 1 ; 1 R`).
        create.shell = Some(script_shell(
            "read go; stty -icanon -echo; printf '\\033[6n'; \
             dd bs=1 count=6 of=/dev/null 2>/dev/null; echo replied-$((6*7)); sleep 60",
        ));
        "replied-42".to_owned()
    };
    let info = manager.create(create).expect("create");

    // A client that reads raw output from the start and never answers.
    let mut silent = manager.attach(&info.id, Some(0)).expect("attach");
    if cfg!(unix) {
        manager.input(&info.id, &line("go")).expect("input");
    }
    let mut raw = Vec::new();
    let deadline = Instant::now() + TIMEOUT;
    while !raw.windows(4).any(|window| window == b"\x1b[6n") {
        assert!(Instant::now() < deadline, "no cursor query in {raw:?}");
        match tokio::time::timeout(TIMEOUT, silent.next()).await {
            Ok(Some(AttachItem::Output { bytes, .. })) => raw.extend_from_slice(&bytes),
            Ok(Some(_)) => {}
            other => panic!("unexpected {other:?}"),
        }
    }
    drop(silent);

    let expected = if cfg!(windows) {
        let (command, expected) = marker_command("answered");
        manager.input(&info.id, &command).expect("input");
        expected
    } else {
        expected
    };
    wait_for_screen(&manager, &info.id, &expected).await;
}
