use std::path::Path;

use massorca_core_git::{Git, RebaseOutcome};

use crate::fixture::{Fixture, commit_file, rev, run_git};

fn git() -> Git {
    Git::discover().expect("git on PATH")
}

/// Gives the fixture repository the identity the commits a rebase replays are made with.
fn with_identity(repo: &Path) {
    run_git(repo, &["config", "user.name", "t"]);
    run_git(repo, &["config", "user.email", "t@example.invalid"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);
}

#[test]
fn a_clean_rebase_moves_the_branch_onto_the_advanced_base() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    with_identity(&repo);
    run_git(&repo, &["checkout", "-q", "-b", "feature"]);
    commit_file(&repo, "feature.txt", "feature\n", "feature work");
    run_git(&repo, &["checkout", "-q", "main"]);
    commit_file(&repo, "main.txt", "main\n", "main moves on");
    let base_tip = rev(&repo, "main");
    run_git(&repo, &["checkout", "-q", "feature"]);
    let feature_before = rev(&repo, "HEAD");

    let outcome = git().rebase_onto(&repo, "main").expect("rebase runs");

    let head = rev(&repo, "HEAD");
    assert_eq!(outcome, RebaseOutcome::Rebased { head: head.clone() });
    assert_ne!(head, feature_before, "the branch got new commits");
    assert_eq!(rev(&repo, "HEAD~1"), base_tip, "the base is the parent now");
    assert!(repo.join("feature.txt").exists());
    assert!(repo.join("main.txt").exists());
    assert_eq!(
        run_git(&repo, &["branch", "--show-current"]).trim(),
        "feature"
    );
    assert!(!git().status_dirty(&repo).expect("status"));
}

#[test]
fn rebasing_onto_a_base_the_branch_already_contains_changes_nothing() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    with_identity(&repo);
    run_git(&repo, &["checkout", "-q", "-b", "feature"]);
    commit_file(&repo, "feature.txt", "feature\n", "feature work");
    let head_before = rev(&repo, "HEAD");

    let outcome = git().rebase_onto(&repo, "main").expect("rebase runs");

    assert_eq!(outcome, RebaseOutcome::UpToDate);
    assert_eq!(rev(&repo, "HEAD"), head_before);
}

#[test]
fn a_conflicting_rebase_reports_the_paths_and_leaves_head_and_the_worktree_as_they_were() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    with_identity(&repo);
    run_git(&repo, &["checkout", "-q", "-b", "feature"]);
    commit_file(&repo, "README.md", "from feature\n", "feature change");
    run_git(&repo, &["checkout", "-q", "main"]);
    commit_file(&repo, "README.md", "from main\n", "main change");
    run_git(&repo, &["checkout", "-q", "feature"]);
    let head_before = rev(&repo, "HEAD");

    let outcome = git().rebase_onto(&repo, "main").expect("rebase runs");

    assert_eq!(
        outcome,
        RebaseOutcome::Conflict {
            paths: vec!["README.md".to_owned()]
        }
    );
    assert_eq!(rev(&repo, "HEAD"), head_before);
    assert_eq!(
        std::fs::read_to_string(repo.join("README.md")).expect("read README.md"),
        "from feature\n"
    );
    assert!(
        git()
            .operation_in_progress(&repo)
            .expect("operation")
            .is_none()
    );
    assert!(!git().status_dirty(&repo).expect("status"));
}

#[test]
fn a_base_name_that_starts_with_a_dash_is_refused_before_git_runs() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let head_before = rev(&repo, "HEAD");

    let result = git().rebase_onto(&repo, "--root");

    assert!(result.is_err(), "{result:?}");
    assert_eq!(rev(&repo, "HEAD"), head_before);
}
