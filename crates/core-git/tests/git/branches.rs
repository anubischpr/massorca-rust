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
