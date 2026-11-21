//! A failed row insert after git succeeded must leave no worktree, branch or row behind.

use massorca_protocol::codes;

use crate::fixture::{Fixture, branch_exists, create_params, expect_code, run_git, worktree_count};

#[test]
fn a_failed_insert_removes_the_new_worktree_and_branch_and_leaves_no_row() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    fx.fail_workspace_inserts();

    expect_code(
        fx.workspaces
            .create_workspace(create_params(&project.name, "rolled")),
        codes::INTERNAL,
    );

    assert!(!fx.path("defaults/repo/rolled").exists());
    assert!(!branch_exists(&fx.repo(), "rolled"));
    assert_eq!(worktree_count(&fx.repo()), 1);
    assert!(
        fx.workspaces
            .list_workspaces(None)
            .expect("list")
            .is_empty()
    );
}

#[test]
fn after_a_rolled_back_insert_the_same_name_can_be_created_again() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    fx.fail_workspace_inserts();
    expect_code(
        fx.workspaces
            .create_workspace(create_params(&project.name, "again")),
        codes::INTERNAL,
    );
    fx.allow_workspace_inserts();

    let ws = fx.create(&project.name, "again");

    assert!(ws.created_branch);
    assert!(branch_exists(&fx.repo(), "again"));
}

#[test]
fn a_failed_insert_keeps_a_branch_the_request_did_not_create() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    run_git(&fx.repo(), &["branch", "existing"]);
    fx.fail_workspace_inserts();
    let mut params = create_params(&project.name, "existing-wt");
    params.branch = Some("existing".into());
    params.existing_branch = true;

    expect_code(fx.workspaces.create_workspace(params), codes::INTERNAL);

    assert!(branch_exists(&fx.repo(), "existing"));
    assert_eq!(worktree_count(&fx.repo()), 1);
    assert!(!fx.path("defaults/repo/existing-wt").exists());
}
