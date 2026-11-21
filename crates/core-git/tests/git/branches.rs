//! Listing, creating, switching, renaming and deleting local branches, with every refusal of decision 0013.

use std::path::Path;

use massorca_core_git::{BranchError, Git};
use massorca_protocol::codes;

use crate::fixture::{Fixture, commit_file, commit_on_branch, rev, run_git, text};

fn git() -> Git {
    Git::discover().expect("git is installed")
}

fn refusal(error: BranchError) -> &'static str {
    match error {
        BranchError::Refused { code, .. } => code,
        other => panic!("expected a refusal, got {other}"),
    }
}

fn exists(repo: &Path, branch: &str) -> bool {
    git().branch_exists(repo, branch).expect("branch_exists")
}

#[test]
fn branches_list_current_first_with_upstream_tracking_and_other_worktrees() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    commit_on_branch(&repo, "alpha", "alpha.txt");
    run_git(&repo, &["branch", "zeta"]);
    // `beta` tracks `main` locally: one commit ahead, one behind.
    run_git(&repo, &["branch", "--track", "beta", "main"]);
    run_git(&repo, &["checkout", "-q", "beta"]);
    commit_file(&repo, "beta.txt", "b\n", "beta work");
    run_git(&repo, &["checkout", "-q", "main"]);
    commit_file(&repo, "main.txt", "m\n", "main work");
    // `gone` tracks a remote-tracking ref that no longer exists.
    run_git(&repo, &["branch", "gone"]);
    run_git(
        &repo,
        &["remote", "add", "origin", "https://example.invalid/repo"],
    );
    run_git(&repo, &["config", "branch.gone.remote", "origin"]);
    run_git(&repo, &["config", "branch.gone.merge", "refs/heads/gone"]);
    let linked = fixture.sibling("linked");
    run_git(&repo, &["worktree", "add", "-q", text(&linked), "zeta"]);
    run_git(&repo, &["checkout", "-q", "alpha"]);

    let branches = git().branches(&repo).expect("branches");
    let names: Vec<_> = branches.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["alpha", "beta", "gone", "main", "zeta"]);
    let alpha = &branches[0];
    assert!(alpha.current && !alpha.checked_out_elsewhere);
    assert_eq!(alpha.commit, rev(&repo, "alpha"));
    assert_eq!(alpha.upstream, None);
    let beta = &branches[1];
    assert_eq!(beta.upstream.as_deref(), Some("main"));
    assert_eq!((beta.ahead, beta.behind), (Some(1), Some(1)));
    assert!(!beta.upstream_gone && !beta.current);
    assert!(branches[2].upstream_gone);
    assert_eq!((branches[2].ahead, branches[2].behind), (None, None));
    assert!(!branches[3].checked_out_elsewhere);
    assert!(branches[4].checked_out_elsewhere && !branches[4].current);
}

#[test]
fn an_up_to_date_upstream_reports_zero_ahead_and_behind() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["branch", "--track", "even", "main"]);
    let branches = git().branches(&repo).expect("branches");
    let even = branches.iter().find(|b| b.name == "even").expect("even");
    assert_eq!((even.ahead, even.behind), (Some(0), Some(0)));
}

#[test]
fn create_starts_at_head_or_a_revision_without_upstream_and_can_switch() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let first = rev(&repo, "HEAD");
    commit_file(&repo, "two.txt", "2\n", "second");

    let created = git()
        .branch_create(&repo, "from-head", None, false)
        .expect("create");
    assert_eq!(created.commit, rev(&repo, "HEAD"));
    assert!(!created.current && created.upstream.is_none());
    assert_eq!(
        run_git(&repo, &["symbolic-ref", "--short", "HEAD"]).trim_end(),
        "main"
    );

    let switched = git()
        .branch_create(&repo, "from-first", Some(&first), true)
        .expect("create and switch");
    assert_eq!(switched.commit, first);
    assert!(switched.current && switched.upstream.is_none());
    assert_eq!(
        run_git(&repo, &["symbolic-ref", "--short", "HEAD"]).trim_end(),
        "from-first"
    );
    // A remote-tracking style start must not configure an upstream either.
    let tracked = git()
        .branch_create(&repo, "from-main", Some("main"), false)
        .expect("create from a branch");
    assert_eq!(tracked.upstream, None);
}

#[test]
fn create_refuses_invalid_names_existing_branches_and_unknown_starts() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    for name in ["-x", "bad..name", "a b", ""] {
        let error = git()
            .branch_create(&repo, name, None, false)
            .expect_err("invalid");
        assert_eq!(refusal(error), codes::BRANCH_NAME_INVALID, "{name:?}");
    }
    let error = git()
        .branch_create(&repo, "main", None, false)
        .expect_err("exists");
    assert_eq!(refusal(error), codes::BRANCH_EXISTS);
    for start in ["no-such-rev", "--orphan"] {
        let error = git()
            .branch_create(&repo, "new", Some(start), false)
            .expect_err("unknown start");
        assert_eq!(refusal(error), codes::GIT_REVISION_NOT_FOUND);
    }
    assert!(!exists(&repo, "new"));
}

#[test]
fn switch_checks_out_an_existing_branch_and_refuses_the_rest() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    commit_on_branch(&repo, "topic", "topic.txt");
    git().branch_switch(&repo, "topic").expect("switch");
    assert!(repo.join("topic.txt").exists());

    let error = git()
        .branch_switch(&repo, "missing")
        .expect_err("not found");
    assert_eq!(refusal(error), codes::BRANCH_NOT_FOUND);

    run_git(&repo, &["branch", "busy"]);
    let linked = fixture.sibling("linked");
    run_git(&repo, &["worktree", "add", "-q", text(&linked), "busy"]);
    let error = git().branch_switch(&repo, "busy").expect_err("checked out");
    assert_eq!(refusal(error), codes::BRANCH_CHECKED_OUT);
    assert_eq!(
        run_git(&repo, &["symbolic-ref", "--short", "HEAD"]).trim_end(),
        "topic"
    );
}

#[test]
fn switch_refuses_to_overwrite_local_changes_and_lists_the_files() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["checkout", "-q", "-b", "other"]);
    commit_file(&repo, "README.md", "other\n", "change readme");
    commit_file(&repo, "notes.txt", "other\n", "add notes");
    run_git(&repo, &["checkout", "-q", "main"]);
    std::fs::write(repo.join("README.md"), "local edit\n").expect("write");
    std::fs::write(repo.join("notes.txt"), "untracked\n").expect("write");

    match git().branch_switch(&repo, "other") {
        Err(BranchError::WouldOverwrite { paths, .. }) => {
            assert_eq!(paths, ["README.md", "notes.txt"]);
        }
        other => panic!("expected an overwrite refusal, got {other:?}"),
    }
    assert_eq!(
        std::fs::read_to_string(repo.join("README.md"))
            .expect("read")
            .trim_end(),
        "local edit"
    );
}

#[test]
fn rename_moves_a_branch_and_refuses_bad_targets() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["branch", "old"]);
    run_git(&repo, &["branch", "taken"]);

    let renamed = git().branch_rename(&repo, "old", "new").expect("rename");
    assert_eq!(renamed.name, "new");
    assert!(exists(&repo, "new") && !exists(&repo, "old"));

    let error = git()
        .branch_rename(&repo, "missing", "x")
        .expect_err("missing");
    assert_eq!(refusal(error), codes::BRANCH_NOT_FOUND);
    let error = git()
        .branch_rename(&repo, "new", "bad..name")
        .expect_err("invalid");
    assert_eq!(refusal(error), codes::BRANCH_NAME_INVALID);
    let error = git()
        .branch_rename(&repo, "new", "taken")
        .expect_err("exists");
    assert_eq!(refusal(error), codes::BRANCH_EXISTS);
    assert!(exists(&repo, "new") && exists(&repo, "taken"));

    let current = git().branch_rename(&repo, "main", "trunk").expect("rename");
    assert!(current.current);
}

#[test]
fn delete_removes_a_merged_branch_and_returns_its_commit() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["branch", "merged"]);
    let commit = rev(&repo, "merged");
    let deleted = git()
        .branch_delete(&repo, "merged", false, &[])
        .expect("delete");
    assert_eq!(deleted, commit);
    assert!(!exists(&repo, "merged"));
}

#[test]
fn delete_refuses_unmerged_unless_forced() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    commit_on_branch(&repo, "topic", "topic.txt");
    let commit = rev(&repo, "topic");
    let error = git()
        .branch_delete(&repo, "topic", false, &[])
        .expect_err("unmerged");
    assert_eq!(refusal(error), codes::BRANCH_UNMERGED);
    assert!(exists(&repo, "topic"));

    let deleted = git()
        .branch_delete(&repo, "topic", true, &[])
        .expect("forced");
    assert_eq!(deleted, commit);
    assert!(!exists(&repo, "topic"));
}

#[test]
fn delete_always_refuses_checked_out_and_in_use_branches_even_forced() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let error = git()
        .branch_delete(&repo, "main", true, &[])
        .expect_err("current");
    assert_eq!(refusal(error), codes::BRANCH_CHECKED_OUT);

    run_git(&repo, &["branch", "busy"]);
    let linked = fixture.sibling("linked");
    run_git(&repo, &["worktree", "add", "-q", text(&linked), "busy"]);
    let error = git()
        .branch_delete(&repo, "busy", true, &[])
        .expect_err("checked out elsewhere");
    assert_eq!(refusal(error), codes::BRANCH_CHECKED_OUT);

    run_git(&repo, &["branch", "held"]);
    let error = git()
        .branch_delete(&repo, "held", true, &["held".to_owned()])
        .expect_err("in use");
    assert_eq!(refusal(error), codes::BRANCH_IN_USE);

    let error = git()
        .branch_delete(&repo, "missing", true, &[])
        .expect_err("missing");
    assert_eq!(refusal(error), codes::BRANCH_NOT_FOUND);
    assert!(exists(&repo, "busy") && exists(&repo, "held"));
}

#[test]
fn a_forced_delete_keeps_a_branch_that_moved_since_it_was_read() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let stale = rev(&repo, "HEAD");
    commit_on_branch(&repo, "topic", "topic.txt");
    git()
        .delete_branch_at(&repo, "topic", &stale)
        .expect_err("the branch moved");
    assert!(exists(&repo, "topic"));
    git()
        .delete_branch_at(&repo, "topic", &rev(&repo, "topic"))
        .expect("delete at its commit");
    assert!(!exists(&repo, "topic"));
}
