//! Linked worktrees git lists for a project that no workspace records are reported wherever they sit, with git's lock
//! and prune state (GIT-14). Listing never removes one.

use std::path::Path;

use crate::fixture::{Fixture, canonical, run_git, text};

/// Creates a linked worktree at `path` on `branch`. The path is outside the project's workspaces root.
fn add_worktree(fx: &Fixture, path: &Path, branch: &str) {
    std::fs::create_dir_all(path.parent().expect("has a parent")).expect("create the parent");
    run_git(
        &fx.repo(),
        &["worktree", "add", "-b", branch, &text(path), "HEAD"],
    );
}

#[test]
fn a_worktree_outside_the_workspaces_root_is_listed_with_its_lock_reason() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let side = fx.path("elsewhere/side");
    add_worktree(&fx, &side, "side");
    run_git(
        &fx.repo(),
        &[
            "worktree",
            "lock",
            "--reason",
            "agent is busy",
            &text(&side),
        ],
    );

    let listed = fx.workspaces.unregistered_worktrees(None).expect("list");

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].project_id, project.id);
    assert_eq!(listed[0].path, text(&canonical(&side)));
    assert_eq!(listed[0].branch.as_deref(), Some("side"));
    assert_eq!(listed[0].locked.as_deref(), Some("agent is busy"));
    assert_eq!(listed[0].prunable, None);
    assert!(side.is_dir(), "listing never removes a worktree");
}

#[test]
fn a_worktree_whose_folder_is_gone_is_listed_as_prunable() {
    let fx = Fixture::new();
    fx.repo_project();
    let side = fx.path("elsewhere/gone");
    add_worktree(&fx, &side, "gone");
    std::fs::remove_dir_all(&side).expect("remove the folder");

    let listed = fx.workspaces.unregistered_worktrees(None).expect("list");

    assert_eq!(listed.len(), 1);
    assert!(
        listed[0].prunable.is_some(),
        "the missing folder is reported"
    );
    assert_eq!(listed[0].locked, None);
}

#[test]
fn a_repository_with_only_its_main_worktree_lists_nothing() {
    let fx = Fixture::new();
    fx.repo_project();
    assert!(
        fx.workspaces
            .unregistered_worktrees(None)
            .expect("list")
            .is_empty()
    );
}
