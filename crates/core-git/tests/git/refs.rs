use massorca_core_git::{Git, GitError};

use crate::fixture::{Fixture, commit_on_branch, run_git, text};

fn git() -> Git {
    Git::discover().expect("git on PATH")
}

#[test]
fn branch_exists_sees_local_branches_but_not_remote_tracking_refs() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["branch", "feature/login"]);
    run_git(
        &repo,
        &["update-ref", "refs/remotes/origin/only-remote", "HEAD"],
    );
    let git = git();
    assert!(git.branch_exists(&repo, "main").expect("check"));
    assert!(git.branch_exists(&repo, "feature/login").expect("check"));
    assert!(!git.branch_exists(&repo, "missing").expect("check"));
    assert!(!git.branch_exists(&repo, "only-remote").expect("check"));
}

#[test]
fn remote_branch_exists_matches_a_branch_under_any_remote() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["update-ref", "refs/remotes/origin/x", "HEAD"]);
    run_git(
        &repo,
        &["update-ref", "refs/remotes/upstream/feature/y", "HEAD"],
    );
    let git = git();
    assert!(git.remote_branch_exists(&repo, "x").expect("check"));
    assert!(git.remote_branch_exists(&repo, "feature/y").expect("check"));
    assert!(!git.remote_branch_exists(&repo, "y").expect("check"));
    assert!(!git.remote_branch_exists(&repo, "missing").expect("check"));
}

#[test]
fn remote_branch_exists_ignores_local_branches() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["branch", "local-only"]);
    assert!(
        !git()
            .remote_branch_exists(&repo, "local-only")
            .expect("check")
    );
}

#[test]
fn ref_exists_resolves_commits_and_rejects_unknown_revisions() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let git = git();
    assert!(git.ref_exists(&repo, "HEAD").expect("check"));
    assert!(git.ref_exists(&repo, "main").expect("check"));
    assert!(!git.ref_exists(&repo, "no-such-ref").expect("check"));
    assert!(!git.ref_exists(&repo, "--all").expect("check"));
}

#[test]
fn ref_exists_outside_a_repository_is_an_error_not_false() {
    let dir = tempfile::tempdir().expect("temp dir");
    let error = git()
        .ref_exists(dir.path(), "HEAD")
        .expect_err("not a repository");
    assert!(matches!(error, GitError::Failed { .. }), "{error:?}");
}

#[test]
fn check_branch_name_accepts_valid_names_and_rejects_invalid_ones() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let git = git();
    for name in ["feature/login", "release-1.2", "wip"] {
        assert!(
            git.check_branch_name(&repo, name).expect("check"),
            "{name} is valid"
        );
    }
    for name in [
        "",
        "bad..name",
        "with space",
        "ends/",
        "has~tilde",
        "name.lock",
        "-leading-dash",
        "@{-1}",
        "a@{b",
        "HEAD",
    ] {
        assert!(
            !git.check_branch_name(&repo, name).expect("check"),
            "{name} is invalid"
        );
    }
}

#[test]
fn delete_branch_if_merged_deletes_a_merged_branch() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["branch", "done"]);
    let git = git();
    assert!(git.delete_branch_if_merged(&repo, "done").expect("delete"));
    assert!(!git.branch_exists(&repo, "done").expect("check"));
}

#[test]
fn delete_branch_if_merged_keeps_a_branch_with_unmerged_commits() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    commit_on_branch(&repo, "wip", "wip.txt");
    let git = git();
    assert!(!git.delete_branch_if_merged(&repo, "wip").expect("delete"));
    assert!(git.branch_exists(&repo, "wip").expect("check"));
}

#[test]
fn delete_branch_if_merged_reports_a_missing_branch_as_an_error() {
    let fixture = Fixture::new();
    let error = git()
        .delete_branch_if_merged(&fixture.repo(), "missing")
        .expect_err("no such branch");
    assert!(matches!(error, GitError::Failed { .. }), "{error:?}");
}

#[test]
fn delete_branch_if_merged_refuses_a_branch_checked_out_elsewhere() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("busy-wt");
    run_git(
        &repo,
        &["worktree", "add", "-q", "-b", "busy", text(&linked)],
    );
    let error = git()
        .delete_branch_if_merged(&repo, "busy")
        .expect_err("branch is checked out");
    assert!(matches!(error, GitError::Failed { .. }), "{error:?}");
    assert!(git().branch_exists(&repo, "busy").expect("check"));
}
