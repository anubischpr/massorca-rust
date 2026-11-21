//! Driving a running shell in a pseudo-terminal: resize, typed input, signals, and closing.

mod support;

use massorca_core_terminal::pty::{PtyError, PtySize};
use massorca_core_terminal::shell::{EnvSnapshot, resolve_program};
use support::{exit_within, shell_program, spec, start, terminal_env, type_text};

#[test]
fn a_resize_reaches_the_shell_while_it_runs() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut started = spec(
        shell_program(),
        if cfg!(windows) { &["/d"] } else { &["-i"] },
        dir.path(),
        terminal_env(),
    );
    started.size = PtySize {
        cols: 100,
        rows: 30,
    };
    let running = start(&started);

    running
        .master
        .resize(PtySize {
            cols: 120,
            rows: 40,
        })
        .expect("resize the terminal");
    if cfg!(windows) {
        type_text(&running.master, "mode con\r\n");
        // Wait for the prompt after the report too: input typed while `mode con` runs is lost.
        running
            .output
            .wait_until("the console size, then the prompt", |text| {
                numbers_after_colons(text).contains(&40)
                    && numbers_after_colons(text).contains(&120)
                    && text.trim_end().ends_with('>')
            });
        type_text(&running.master, "exit\r\n");
    } else {
        type_text(&running.master, "stty size\n");
        running.output.wait_until("the terminal size", |text| {
            text.lines().any(|line| {
                line.split_whitespace()
                    .collect::<Vec<_>>()
                    .ends_with(&["40", "120"])
            })
        });
        type_text(&running.master, "exit\n");
    }
    let _ = exit_within(&running.exit);
    running.master.close();
    running.output.finish();
}

/// Every integer that follows a colon on a line, like the values of `mode con`.
fn numbers_after_colons(text: &str) -> Vec<u32> {
    text.lines()
        .filter_map(|line| line.rsplit(':').next())
        .filter_map(|tail| tail.trim().parse().ok())
        .collect()
}

#[test]
fn typed_input_reaches_the_interactive_shell() {
    let dir = tempfile::tempdir().expect("temp dir");
    let running = start(&spec(
        shell_program(),
        if cfg!(windows) { &["/d"] } else { &["-i"] },
        dir.path(),
        terminal_env(),
    ));

    if cfg!(windows) {
        type_text(&running.master, "echo typed-marker\r\n");
        running.output.wait_until("the echoed marker", |text| {
            text.matches("typed-marker").count() >= 2
        });
        type_text(&running.master, "exit\r\n");
    } else {
        type_text(&running.master, "echo typed-marker\n");
        running.output.wait_until("the echoed marker", |text| {
            text.matches("typed-marker").count() >= 2
        });
        type_text(&running.master, "exit\n");
    }
    let _ = exit_within(&running.exit);
    running.master.close();
    running.output.finish();
}

#[cfg(unix)]
#[test]
fn a_signal_death_is_reported_as_a_signal() {
    let dir = tempfile::tempdir().expect("temp dir");
    let running = start(&spec(
        shell_program(),
        &["-c", "kill -KILL $$"],
        dir.path(),
        terminal_env(),
    ));

    let exit = exit_within(&running.exit);
    running.master.close();
    running.output.finish();

    assert_eq!(exit.code, None, "{exit:?}");
    assert!(exit.signal.is_some(), "{exit:?}");
}

#[cfg(unix)]
#[test]
fn the_shell_is_the_foreground_group_of_its_terminal() {
    let dir = tempfile::tempdir().expect("temp dir");
    let running = start(&spec(shell_program(), &["-i"], dir.path(), terminal_env()));
    let pid = running.master.pid().expect("the shell has a pid");

    assert_eq!(
        running.master.foreground_process_group(),
        i32::try_from(pid).ok(),
        "an interactive shell leads its own session and foreground group"
    );
    type_text(&running.master, "exit\n");
    let _ = exit_within(&running.exit);
    running.master.close();
    running.output.finish();
}

#[test]
fn writes_and_resizes_fail_once_the_terminal_is_closed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let args: &[&str] = if cfg!(windows) {
        &["/d", "/c", "ping -n 60 127.0.0.1 >nul"]
    } else {
        &["60"]
    };
    let program = if cfg!(windows) {
        shell_program()
    } else {
        resolve_program("sleep", &EnvSnapshot::current()).expect("sleep is on PATH")
    };
    let running = start(&spec(program, args, dir.path(), terminal_env()));

    running.master.close();
    running.master.close();

    assert!(running.master.write_all(b"x").is_err());
    assert!(matches!(
        running.master.resize(PtySize { cols: 80, rows: 24 }),
        Err(PtyError::Resize(_))
    ));
}
