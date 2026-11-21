//! Spawning real shells in pseudo-terminals (decision 0030): output, exit, cwd, env, refusals.

mod support;

use std::path::{Path, PathBuf};

use massorca_core_terminal::pty::{ExitInfo, PtyChild, PtyError, PtyMaster, PtyReader, spawn};
use massorca_core_terminal::shell::{EnvSnapshot, resolve_program};
use support::{exit_within, shell_program, spec, start, terminal_env, visible_lines};

#[test]
fn output_and_exit_code_come_back_from_the_shell() {
    let dir = tempfile::tempdir().expect("temp dir");
    let args: &[&str] = if cfg!(windows) {
        &["/d", "/c", "echo hello-pty & exit 3"]
    } else {
        &["-c", "printf hello-pty; exit 3"]
    };
    let running = start(&spec(shell_program(), args, dir.path(), terminal_env()));

    let exit = exit_within(&running.exit);
    running.output.wait_for("hello-pty");
    running.master.close();
    running.output.finish();

    assert_eq!(
        exit,
        ExitInfo {
            code: Some(3),
            signal: None
        }
    );
}

#[test]
fn the_working_directory_is_the_requested_one() {
    let dir = tempfile::tempdir().expect("temp dir");
    let expected = dir.path().canonicalize().expect("canonical temp dir");
    let args: &[&str] = if cfg!(windows) {
        &["/d", "/c", "cd"]
    } else {
        &["-c", "pwd -P"]
    };
    let running = start(&spec(shell_program(), args, dir.path(), terminal_env()));

    running.output.wait_until("the working directory", |text| {
        text.lines().any(|line| {
            Path::new(line.trim()).canonicalize().ok().as_deref() == Some(expected.as_path())
        })
    });
    running.master.close();
    running.output.finish();
}

#[test]
fn the_environment_is_exactly_the_one_passed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let env = vec![(
        "MASSORCA_TEST_MARKER".to_owned(),
        "visible-marker".to_owned(),
    )];
    let (program, args): (PathBuf, &[&str]) = if cfg!(windows) {
        (shell_program(), &["/d", "/c", "set"])
    } else {
        (
            resolve_program("env", &EnvSnapshot::current()).expect("env is on PATH"),
            &[],
        )
    };
    let running = start(&spec(program, args, dir.path(), env));

    running
        .output
        .wait_for("MASSORCA_TEST_MARKER=visible-marker");
    let _ = exit_within(&running.exit);
    running.master.close();
    running.output.finish();

    let lines = visible_lines(&running.output.raw());
    let inherited = |name: &str| {
        lines
            .iter()
            .any(|line| line.to_ascii_uppercase().starts_with(&format!("{name}=")))
    };
    assert!(
        !inherited("PATH"),
        "the test process PATH leaked into the terminal"
    );
    assert!(!inherited("USERPROFILE") && !inherited("HOME"));
}

#[test]
fn a_relative_program_is_refused_before_anything_starts() {
    let dir = tempfile::tempdir().expect("temp dir");
    let relative = spec(PathBuf::from("sh"), &[], dir.path(), terminal_env());
    assert!(matches!(
        spawn(&relative),
        Err(PtyError::ProgramNotAbsolute(_))
    ));
}

#[test]
fn a_missing_working_directory_is_refused_before_anything_starts() {
    let dir = tempfile::tempdir().expect("temp dir");
    let missing = dir.path().join("not-created");
    let refused = spec(shell_program(), &[], &missing, terminal_env());
    assert!(matches!(spawn(&refused), Err(PtyError::CwdNotFound(_))));
}

#[test]
fn the_handles_can_move_between_threads() {
    fn assert_send<T: Send>() {}
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PtyMaster>();
    assert_send::<PtyReader>();
    assert_send::<PtyChild>();
}
