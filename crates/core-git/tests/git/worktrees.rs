use std::path::Path;

use massorca_core_git::{Git, GitError, GitVersion, WorktreeAdd, WorktreeEntry};

use crate::fixture::{Fixture, canonical, commit_file, commit_on_branch, rev, run_git, text};

fn git() -> Git {
    Git::discover().expect("git on PATH")
}

/// The entry whose path is `path`, compared as canonical paths.
fn entry_at<'a>(entries: &'a [WorktreeEntry], path: &Path) -> &'a WorktreeEntry {
    let wanted = canonical(path);
    entries
        .iter()
        .find(|entry| canonical(&entry.path) == wanted)
        .unwrap_or_else(|| panic!("no worktree at {}", path.display()))
}

/// Git 2.31 added the `locked` and `prunable` lines to `worktree list --porcelain`; older
/// git prints neither, so only newer git can report lock reasons or prune state.
fn porcelain_reports_lock_and_prune_state(git: &Git) -> bool {
    let since = GitVersion {
        major: 2,
        minor: 31,
        patch: 0,
    };
    git.version().expect("git --version") >= since
}

fn add_new_branch(repo: &Path, path: &Path, branch: &str) {
    git()
        .worktree_add(repo, path, WorktreeAdd::NewBranch { branch, base: None })
        .expect("worktree add");
}

#[test]
fn add_creates_a_new_branch_from_head() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("feature-wt");

    add_new_branch(&repo, &linked, "feature");

    assert!(linked.join("README.md").is_file());
    assert_eq!(rev(&linked, "HEAD"), rev(&repo, "main"));
    assert!(git().branch_exists(&repo, "feature").expect("branch check"));
    let entries = git().worktree_list(&repo).expect("list");
    assert_eq!(
        entry_at(&entries, &linked).branch.as_deref(),
        Some("feature")
    );
}

#[test]
fn add_with_an_explicit_base_starts_the_branch_at_that_base() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("from-base");
    let base = rev(&repo, "HEAD");
    run_git(&repo, &["branch", "old-base", base.as_str()]);
    commit_file(&repo, "later.txt", "later\n", "later commit");

    git()
        .worktree_add(
            &repo,
            &linked,
            WorktreeAdd::NewBranch {
                branch: "from-old",
                base: Some("old-base"),
            },
        )
        .expect("worktree add");

    assert_eq!(rev(&linked, "HEAD"), base);
    assert!(!linked.join("later.txt").exists());
}

#[test]
fn add_with_an_existing_branch_checks_that_branch_out() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("existing-wt");
    commit_on_branch(&repo, "existing", "existing.txt");
    let tip = rev(&repo, "existing");

    git()
        .worktree_add(
            &repo,
            &linked,
            WorktreeAdd::ExistingBranch { branch: "existing" },
        )
        .expect("worktree add");

    assert_eq!(rev(&linked, "HEAD"), tip);
    assert!(linked.join("existing.txt").is_file());
    let entries = git().worktree_list(&repo).expect("list");
    assert_eq!(
        entry_at(&entries, &linked).branch.as_deref(),
        Some("existing")
    );
}

#[test]
fn add_with_an_existing_branch_checked_out_elsewhere_fails() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    commit_on_branch(&repo, "busy", "busy.txt");
    git()
        .worktree_add(
            &repo,
            &fixture.sibling("first"),
            WorktreeAdd::ExistingBranch { branch: "busy" },
        )
        .expect("first add");

    let error = git()
        .worktree_add(
            &repo,
            &fixture.sibling("second"),
            WorktreeAdd::ExistingBranch { branch: "busy" },
        )
        .expect_err("branch is already checked out");
    assert!(matches!(error, GitError::Failed { .. }), "{error:?}");
}

#[test]
fn add_new_branch_fails_when_the_branch_already_exists() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["branch", "taken"]);

    let error = git()
        .worktree_add(
            &repo,
            &fixture.sibling("taken-wt"),
            WorktreeAdd::NewBranch {
                branch: "taken",
                base: None,
            },
        )
        .expect_err("branch exists");
    assert!(matches!(error, GitError::Failed { .. }), "{error:?}");
}

#[test]
fn add_with_a_missing_existing_branch_fails() {
    let fixture = Fixture::new();
    let error = git()
        .worktree_add(
            &fixture.repo(),
            &fixture.sibling("ghost-wt"),
            WorktreeAdd::ExistingBranch { branch: "ghost" },
        )
        .expect_err("no such branch");
    assert!(matches!(error, GitError::Failed { .. }), "{error:?}");
}

#[test]
fn add_refuses_a_path_with_a_line_break_before_git_runs() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    for name in ["line\nbreak", "carriage\rreturn"] {
        let error = git()
            .worktree_add(
                &repo,
                &fixture.sibling(name),
                WorktreeAdd::NewBranch {
                    branch: "never",
                    base: None,
                },
            )
            .expect_err("a line break in the path");
        assert!(
            matches!(error, GitError::UnsupportedPath { .. }),
            "{error:?}"
        );
    }
    assert!(!git().branch_exists(&repo, "never").expect("branch check"));
    assert_eq!(git().worktree_list(&repo).expect("list").len(), 1);
}

#[test]
fn list_of_a_fresh_repository_is_the_main_worktree_alone() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let entries = git().worktree_list(&repo).expect("list");
    assert_eq!(entries.len(), 1);
    let main = &entries[0];
    assert_eq!(canonical(&main.path), canonical(&repo));
    assert_eq!(main.branch.as_deref(), Some("main"));
    assert!(main.head.is_some());
    assert!(!main.bare);
    assert!(!main.detached);
    assert_eq!(main.locked, None);
    assert_eq!(main.prunable, None);
}

#[test]
fn list_shows_the_main_worktree_first_and_every_linked_one_after_it() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let first = fixture.sibling("first-wt");
    let second = fixture.sibling("second-wt");
    add_new_branch(&repo, &first, "first");
    add_new_branch(&repo, &second, "second");

    let entries = git().worktree_list(&repo).expect("list");
    assert_eq!(entries.len(), 3);
    assert_eq!(canonical(&entries[0].path), canonical(&repo));
    assert_eq!(entry_at(&entries, &first).branch.as_deref(), Some("first"));
    assert_eq!(
        entry_at(&entries, &second).branch.as_deref(),
        Some("second")
    );
}

#[test]
fn paths_with_spaces_reach_git_intact() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("with space");
    add_new_branch(&repo, &linked, "spaced");

    let entries = git().worktree_list(&repo).expect("list");
    assert_eq!(
        entry_at(&entries, &linked).branch.as_deref(),
        Some("spaced")
    );
}

#[test]
fn remove_deletes_a_clean_worktree() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("clean-wt");
    add_new_branch(&repo, &linked, "clean");

    git()
        .worktree_remove(&repo, &linked, false)
        .expect("remove clean worktree");

    assert!(!linked.exists());
    assert_eq!(git().worktree_list(&repo).expect("list").len(), 1);
}

#[test]
fn remove_refuses_a_dirty_worktree_without_force_and_removes_it_with_force() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("dirty-wt");
    add_new_branch(&repo, &linked, "dirty");
    std::fs::write(linked.join("scratch.txt"), "unsaved\n").expect("write file");

    let error = git()
        .worktree_remove(&repo, &linked, false)
        .expect_err("dirty worktree");
    assert!(matches!(error, GitError::Failed { .. }), "{error:?}");
    assert!(linked.is_dir());

    git()
        .worktree_remove(&repo, &linked, true)
        .expect("forced remove");
    assert!(!linked.exists());
}

#[test]
fn remove_refuses_a_locked_worktree_even_with_force() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("locked-wt");
    add_new_branch(&repo, &linked, "locked");
    run_git(
        &repo,
        &["worktree", "lock", "--reason", "in use", text(&linked)],
    );

    let error = git()
        .worktree_remove(&repo, &linked, true)
        .expect_err("locked worktree");
    assert!(matches!(error, GitError::Failed { .. }), "{error:?}");
    assert!(linked.is_dir());
}

#[test]
fn list_reports_a_locked_worktree_with_its_reason() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("busy-wt");
    add_new_branch(&repo, &linked, "busy");
    run_git(
        &repo,
        &[
            "worktree",
            "lock",
            "--reason",
            "agent running",
            text(&linked),
        ],
    );

    let entries = git().worktree_list(&repo).expect("list");
    assert_eq!(
        entry_at(&entries, &linked).locked.as_deref(),
        Some("agent running")
    );
}

#[test]
fn list_reports_a_lock_without_a_reason_as_an_empty_reason() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("quiet-wt");
    add_new_branch(&repo, &linked, "quiet");
    run_git(&repo, &["worktree", "lock", text(&linked)]);

    let entries = git().worktree_list(&repo).expect("list");
    assert_eq!(entry_at(&entries, &linked).locked.as_deref(), Some(""));
}

#[test]
fn list_reports_a_lock_on_a_worktree_whose_directory_is_gone() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("locked-gone-wt");
    add_new_branch(&repo, &linked, "locked-gone");
    run_git(
        &repo,
        &["worktree", "lock", "--reason", "kept", text(&linked)],
    );
    std::fs::remove_dir_all(&linked).expect("delete the directory behind git's back");

    let entries = git().worktree_list(&repo).expect("list");
    let entry = entries
        .iter()
        .find(|entry| entry.branch.as_deref() == Some("locked-gone"))
        .expect("the locked worktree is still listed");
    assert_eq!(entry.locked.as_deref(), Some("kept"));
}

#[test]
fn list_reports_a_detached_worktree_without_a_branch() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("detached-wt");
    run_git(
        &repo,
        &["worktree", "add", "-q", "--detach", text(&linked), "HEAD"],
    );

    let entries = git().worktree_list(&repo).expect("list");
    let entry = entry_at(&entries, &linked);
    assert!(entry.detached);
    assert_eq!(entry.branch, None);
    assert!(entry.head.is_some());
}

#[test]
fn list_reports_an_unlocked_worktree_whose_directory_is_gone_as_prunable() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("gone-wt");
    add_new_branch(&repo, &linked, "gone");
    std::fs::remove_dir_all(&linked).expect("delete the directory behind git's back");

    let git = git();
    let entries = git.worktree_list(&repo).expect("list");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1].branch.as_deref(), Some("gone"));
    if porcelain_reports_lock_and_prune_state(&git) {
        assert!(entries[1].prunable.is_some());
    } else {
        assert_eq!(entries[1].prunable, None);
    }
}
