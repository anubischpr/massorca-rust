//! Refusals before anything starts, and the handles' thread safety (decision 0030).

mod support;

use std::path::PathBuf;

use massorca_core_terminal::pty::{PtyChild, PtyError, PtyMaster, PtyReader, spawn};
use support::{shell_program, spec, terminal_env};

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
