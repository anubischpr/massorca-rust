//! A create that stops after `worktree add` and before its row is stored leaves a worktree
//! git registers but no row records (ADR 0008). The same request adopts it.

use std::path::Path;

use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{WorkspaceKind, WorkspaceState};

use crate::fixture::{
    Fixture, branch_exists, canonical, create_params, expect_code, run_git, text, worktree_count,
};

/// Simulates the crash: git registers a worktree at `path` on a new `branch`, nothing else runs.
fn crash_after_worktree_add(fx: &Fixture, path: &Path, branch: &str) {
    std::fs::create_dir_all(path.parent().expect("has a parent")).expect("create root");
    run_git(
        &fx.repo(),
        &["worktree", "add", "-b", branch, &text(path), "HEAD"],
    );
}

#[test]
fn the_same_request_adopts_a_worktree_an_interrupted_create_left_behind() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let target = Path::new(&project.workspaces_root).join("resume");
    crash_after_worktree_add(&fx, &target, "resume");

    let ws = fx.create(&project.name, "resume");

    assert_eq!(ws.kind, WorkspaceKind::Worktree);
    assert_eq!(ws.state, WorkspaceState::Ready);
    assert_eq!(ws.branch.as_deref(), Some("resume"));
    assert!(!ws.created_branch, "the adopted branch is never deleted");
    assert_eq!(ws.path, text(&canonical(&target)));
    assert_eq!(
        fx.workspaces.show_workspace("resume").expect("show").id,
        ws.id
    );
    assert_eq!(fx.workspaces.list_workspaces(None).expect("list").len(), 1);
}

#[test]
fn an_adopted_workspace_is_removed_like_any_other_and_keeps_its_branch() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let target = Path::new(&project.workspaces_root).join("resume");
    crash_after_worktree_add(&fx, &target, "resume");
    fx.create(&project.name, "resume");

    let removed = fx.remove("resume");

    assert!(!removed.branch_deleted);
    assert!(!target.exists());
    assert!(branch_exists(&fx.repo(), "resume"));
}

#[test]
fn a_registered_worktree_on_another_branch_at_the_path_is_still_taken() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let target = Path::new(&project.workspaces_root).join("taken");
    crash_after_worktree_add(&fx, &target, "someone-else");

    expect_code(
        fx.workspaces
            .create_workspace(create_params(&project.name, "taken")),
        codes::WORKSPACE_PATH_TAKEN,
    );
    assert!(target.is_dir(), "a registered worktree is never touched");
}

#[test]
fn the_main_worktree_is_never_adopted_even_when_its_branch_matches() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let mut params = create_params(&project.name, "main-again");
    params.path = Some(project.path.clone());
    params.branch = Some("main".to_owned());

    expect_code(
        fx.workspaces.create_workspace(params),
        codes::WORKSPACE_PATH_TAKEN,
    );
    assert!(fx.repo().join("README.md").exists());
    assert_eq!(fx.workspaces.list_workspaces(None).expect("list").len(), 0);
}

#[test]
fn a_worktree_left_by_an_interrupted_create_is_reported_until_the_create_adopts_it() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let target = Path::new(&project.workspaces_root).join("lost");
    crash_after_worktree_add(&fx, &target, "lost");

    let unregistered = fx.workspaces.unregistered_worktrees(None).expect("list");
    assert_eq!(unregistered.len(), 1);
    assert_eq!(unregistered[0].project_id, project.id);
    assert_eq!(unregistered[0].path, text(&canonical(&target)));
    assert_eq!(unregistered[0].branch.as_deref(), Some("lost"));
    assert!(
        fx.workspaces
            .list_workspaces(None)
            .expect("list")
            .is_empty()
    );

    fx.create(&project.name, "lost");
    assert!(
        fx.workspaces
            .unregistered_worktrees(None)
            .expect("list")
            .is_empty()
    );
}

#[test]
fn reporting_an_unregistered_worktree_removes_nothing() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let target = Path::new(&project.workspaces_root).join("kept");
    crash_after_worktree_add(&fx, &target, "kept");

    fx.workspaces.unregistered_worktrees(None).expect("first");
    fx.workspaces.unregistered_worktrees(None).expect("second");

    assert!(target.join("README.md").exists());
    assert_eq!(worktree_count(&fx.repo()), 2);
    assert!(branch_exists(&fx.repo(), "kept"));
}

#[test]
fn a_worktree_outside_the_workspaces_root_is_reported_too() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let outside = fx.path("elsewhere/outside");
    std::fs::create_dir_all(outside.parent().expect("has a parent")).expect("create parent");
    run_git(
        &fx.repo(),
        &["worktree", "add", "-b", "outside", &text(&outside), "HEAD"],
    );

    let unregistered = fx.workspaces.unregistered_worktrees(None).expect("list");
    assert_eq!(unregistered.len(), 1);
    assert_eq!(unregistered[0].project_id, project.id);
    assert_eq!(unregistered[0].path, text(&canonical(&outside)));
    assert_eq!(unregistered[0].branch.as_deref(), Some("outside"));
}
