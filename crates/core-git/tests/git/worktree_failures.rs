//! A failed or killed `worktree add` leaves nothing behind that it created, and
//! mutations are not cut short by the query timeout.

use std::time::{Duration, Instant};

use massorca_core_git::{Git, GitError, WorktreeAdd};

use crate::fixture::{
    Fixture, canonical, commit_on_branch, install_post_checkout_hook, remove_post_checkout_hook,
    rev,
};

fn git() -> Git {
    Git::discover().expect("git on PATH")
}

fn new_branch(branch: &str) -> WorktreeAdd<'_> {
    WorktreeAdd::NewBranch { branch, base: None }
}

/// Canonical paths of every registered worktree.
fn registered(git: &Git, repo: &std::path::Path) -> Vec<std::path::PathBuf> {
    git.worktree_list(repo)
        .expect("list")
        .iter()
        .map(|entry| canonical(&entry.path))
        .collect()
}

#[test]
fn a_failed_add_removes_the_worktree_and_the_branch_it_created() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("hook-fails");
    // Git reports this failure after it has registered the worktree and created the branch.
    install_post_checkout_hook(&repo, "exit 3");

    let error = git()
        .worktree_add(&repo, &linked, new_branch("doomed"))
        .expect_err("the hook fails");

    assert!(
        matches!(error, GitError::Failed { code: Some(3), .. }),
        "{error:?}"
    );
    assert_eq!(registered(&git(), &repo), vec![canonical(&repo)]);
    assert!(!git().branch_exists(&repo, "doomed").expect("branch check"));
    assert!(!linked.exists(), "the worktree directory was left behind");

    remove_post_checkout_hook(&repo);
    git()
        .worktree_add(&repo, &linked, new_branch("doomed"))
        .expect("a retry succeeds");
}

#[test]
fn a_failed_add_of_an_existing_branch_removes_the_worktree_and_keeps_the_branch() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("existing-hook-fails");
    commit_on_branch(&repo, "kept", "kept.txt");
    let tip = rev(&repo, "kept");
    install_post_checkout_hook(&repo, "exit 3");

    git()
        .worktree_add(
            &repo,
            &linked,
            WorktreeAdd::ExistingBranch { branch: "kept" },
        )
        .expect_err("the hook fails");

    assert_eq!(registered(&git(), &repo), vec![canonical(&repo)]);
    assert_eq!(rev(&repo, "kept"), tip);
}

#[test]
fn a_failed_add_never_removes_a_worktree_registered_before_it() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("already-there");
    git()
        .worktree_add(&repo, &linked, new_branch("first"))
        .expect("first add");

    git()
        .worktree_add(&repo, &linked, new_branch("second"))
        .expect_err("the path is taken");

    assert_eq!(
        registered(&git(), &repo),
        vec![canonical(&repo), canonical(&linked)]
    );
    assert!(git().branch_exists(&repo, "first").expect("branch check"));
    assert!(linked.join("README.md").is_file());
}

#[test]
fn mutations_are_not_bound_by_the_query_timeout() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("no-query-timeout");
    let git = git().with_timeout(Duration::ZERO);

    git.worktree_add(&repo, &linked, new_branch("mutated"))
        .expect("worktree add");
    git.worktree_remove(&repo, &linked, false)
        .expect("worktree remove");
    assert!(
        git.delete_branch_if_merged(&repo, "mutated")
            .expect("branch -d")
    );
}

#[test]
fn a_mutation_timeout_applies_to_mutations() {
    let fixture = Fixture::new();
    let git = git().with_mutation_timeout(Some(Duration::ZERO));

    let error = git
        .worktree_add(
            &fixture.repo(),
            &fixture.sibling("cut-short"),
            new_branch("cut"),
        )
        .expect_err("no time to run");
    assert!(matches!(error, GitError::Timeout { .. }), "{error:?}");
}

#[test]
fn a_hook_left_running_in_the_background_does_not_turn_a_success_into_a_timeout() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("background-hook");
    // The background `sleep` inherits git's output pipes and outlives git.
    install_post_checkout_hook(&repo, "sleep 60 &\nexit 0");
    let started = Instant::now();

    git()
        .worktree_add(&repo, &linked, new_branch("background"))
        .expect("worktree add");

    assert!(started.elapsed() < Duration::from_secs(30));
    assert!(linked.join("README.md").is_file());
}

#[test]
fn a_killed_add_kills_its_hook_and_removes_what_it_created() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("killed-add");
    install_post_checkout_hook(&repo, "sleep 60");
    let git = git().with_mutation_timeout(Some(Duration::from_secs(3)));
    let started = Instant::now();

    let error = git
        .worktree_add(&repo, &linked, new_branch("killed"))
        .expect_err("the hook outlives the timeout");

    assert!(matches!(error, GitError::Timeout { .. }), "{error:?}");
    assert!(started.elapsed() < Duration::from_secs(30));
    assert_eq!(registered(&git, &repo), vec![canonical(&repo)]);
    assert!(!git.branch_exists(&repo, "killed").expect("branch check"));
}
