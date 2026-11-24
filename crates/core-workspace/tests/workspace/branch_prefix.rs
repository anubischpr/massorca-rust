//! The branch prefix of new workspaces: where it comes from and when it applies (ADR 0008).

use std::path::Path;

use massorca_core_workspace::BranchPrefix;
use massorca_protocol::codes;

use crate::fixture::{Fixture, branch_exists, canonical, create_params, expect_code, run_git};

#[test]
fn a_custom_prefix_is_applied_to_the_branch_of_a_new_workspace_and_not_to_its_name_or_folder() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let workspaces = fx
        .workspaces
        .with_branch_prefix(BranchPrefix::Custom(" team/ ".to_owned()));

    let ws = workspaces
        .create_workspace(create_params(&project.name, "feature"))
        .expect("create");

    assert_eq!(ws.name, "feature");
    assert_eq!(ws.branch.as_deref(), Some("team/feature"));
    assert!(branch_exists(&fx.repo(), "team/feature"));
    assert_eq!(
        canonical(Path::new(&ws.path)),
        canonical(&fx.path("defaults/repo/feature"))
    );
}

#[test]
fn a_branch_the_caller_names_is_used_as_given_without_the_prefix() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let workspaces = fx
        .workspaces
        .with_branch_prefix(BranchPrefix::Custom("team".to_owned()));
    let mut params = create_params(&project.name, "explicit");
    params.branch = Some("exact/name".to_owned());

    let ws = workspaces.create_workspace(params).expect("create");

    assert_eq!(ws.branch.as_deref(), Some("exact/name"));
    assert!(branch_exists(&fx.repo(), "exact/name"));
}

#[test]
fn a_custom_prefix_git_refuses_is_refused_before_anything_is_created() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let workspaces = fx
        .workspaces
        .with_branch_prefix(BranchPrefix::Custom("bad..prefix".to_owned()));

    expect_code(
        workspaces.create_workspace(create_params(&project.name, "feature")),
        codes::CONFIG_INVALID_VALUE,
    );
    assert!(!fx.path("defaults").exists());
    assert!(!branch_exists(&fx.repo(), "feature"));
}

#[test]
fn the_git_username_prefix_is_the_repository_github_user() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    run_git(&fx.repo(), &["config", "github.user", "octo-user"]);
    let workspaces = fx.workspaces.with_branch_prefix(BranchPrefix::GitUsername);

    let ws = workspaces
        .create_workspace(create_params(&project.name, "feature"))
        .expect("create");

    assert_eq!(ws.branch.as_deref(), Some("octo-user/feature"));
}

#[test]
fn a_git_username_git_would_not_take_as_a_segment_gives_no_prefix() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    // Empty login keys shadow any global login, so only the repository's user.name is read.
    run_git(&fx.repo(), &["config", "github.user", ""]);
    run_git(&fx.repo(), &["config", "user.username", ""]);
    run_git(&fx.repo(), &["config", "user.name", "Test Person"]);
    let workspaces = fx.workspaces.with_branch_prefix(BranchPrefix::GitUsername);

    let ws = workspaces
        .create_workspace(create_params(&project.name, "feature"))
        .expect("create");

    assert_eq!(ws.branch.as_deref(), Some("feature"));
}
