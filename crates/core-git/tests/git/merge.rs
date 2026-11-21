use std::path::Path;

use massorca_core_git::{Git, GitError, MergeOutcome};

use crate::fixture::{Fixture, commit_file, commit_on_branch, rev, run_git};

fn git() -> Git {
    Git::discover().expect("git on PATH")
}

/// Gives the fixture repository the identity of the merge commits these tests make.
fn with_identity(repo: &Path) {
    run_git(repo, &["config", "user.name", "t"]);
    run_git(repo, &["config", "user.email", "t@example.invalid"]);
    run_git(repo, &["config", "commit.gpgsign", "false"]);
}

#[test]
fn a_clean_merge_creates_a_merge_commit_with_two_parents() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    with_identity(&repo);
    commit_on_branch(&repo, "feature", "feature.txt");
    // main moves on, so the merge cannot be a fast-forward.
    commit_file(&repo, "main.txt", "main\n", "main commit");
    let main_tip = rev(&repo, "HEAD");
    let feature_tip = rev(&repo, "feature");

    let outcome = git().merge_no_ff(&repo, "feature").expect("merge");

    let merged = rev(&repo, "HEAD");
    assert_eq!(
        outcome,
        MergeOutcome::Merged {
            commit: merged.clone()
        }
    );
    let parents = run_git(&repo, &["rev-list", "--parents", "-n", "1", "HEAD"]);
    let ids: Vec<&str> = parents.split_whitespace().collect();
    assert_eq!(
        ids,
        [merged.as_str(), main_tip.as_str(), feature_tip.as_str()]
    );
    assert!(repo.join("feature.txt").exists());
    assert_eq!(run_git(&repo, &["branch", "--show-current"]).trim(), "main");
}

#[test]
fn a_conflicting_merge_reports_the_paths_and_leaves_head_and_the_worktree_as_they_were() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    with_identity(&repo);
    run_git(&repo, &["checkout", "-q", "-b", "feature"]);
    commit_file(&repo, "README.md", "from feature\n", "feature change");
    run_git(&repo, &["checkout", "-q", "main"]);
    commit_file(&repo, "README.md", "from main\n", "main change");
    let head_before = rev(&repo, "HEAD");

    let outcome = git().merge_no_ff(&repo, "feature").expect("merge runs");

    assert_eq!(
        outcome,
        MergeOutcome::Conflict {
            paths: vec!["README.md".to_owned()]
        }
    );
    assert_eq!(rev(&repo, "HEAD"), head_before);
    assert_eq!(
        std::fs::read_to_string(repo.join("README.md")).expect("read README.md"),
        "from main\n"
    );
    assert!(!repo.join(".git").join("MERGE_HEAD").exists());
    assert!(!git().status_dirty(&repo).expect("status"));
}

#[test]
fn merging_a_branch_already_contained_in_head_returns_the_unchanged_head() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    with_identity(&repo);
    run_git(&repo, &["branch", "old"]);
    commit_file(&repo, "second.txt", "second\n", "second commit");
    let head = rev(&repo, "HEAD");

    let outcome = git().merge_no_ff(&repo, "old").expect("merge");

    assert_eq!(
        outcome,
        MergeOutcome::Merged {
            commit: head.clone()
        }
    );
    assert_eq!(rev(&repo, "HEAD"), head);
}

#[test]
fn a_dash_led_name_or_a_missing_branch_is_an_error_that_changes_nothing() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    with_identity(&repo);
    let head = rev(&repo, "HEAD");
    let git = git();
    assert!(matches!(
        git.merge_no_ff(&repo, "--all"),
        Err(GitError::Failed { .. })
    ));
    assert!(matches!(
        git.merge_no_ff(&repo, "missing"),
        Err(GitError::Failed { .. })
    ));
    assert_eq!(rev(&repo, "HEAD"), head);
}
