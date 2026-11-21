use std::time::Duration;

use massorca_core_git::{Git, GitError, GitVersion};

#[test]
fn discover_finds_a_git_at_or_above_the_2_25_baseline() {
    let git = Git::discover().expect("git on PATH");
    let version = git.version().expect("git --version");
    let baseline = GitVersion {
        major: 2,
        minor: 25,
        patch: 0,
    };
    assert!(
        version >= baseline,
        "git {version} is older than the 2.25 baseline"
    );
}

#[test]
fn a_timeout_that_cannot_be_met_reports_the_killed_command() {
    let git = Git::discover()
        .expect("git on PATH")
        .with_timeout(Duration::ZERO);
    match git.version() {
        Err(GitError::Timeout { args }) => assert_eq!(args, vec!["--version"]),
        other => panic!("expected a timeout, got {other:?}"),
    }
}
