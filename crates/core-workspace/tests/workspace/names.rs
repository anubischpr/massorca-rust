use std::path::Path;

use massorca_protocol::codes;

use crate::fixture::{Fixture, branch_exists, canonical, create_params, expect_code};

#[test]
fn names_within_the_rules_are_accepted() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let longest = "x".repeat(64);

    for name in [
        "a",
        "A9",
        "feature.one",
        "snake_case",
        "kebab-case",
        longest.as_str(),
    ] {
        let ws = fx.create(&project.name, name);
        assert_eq!(ws.name, name);
    }
}

#[test]
fn a_name_that_needs_cleaning_is_stored_cleaned_and_its_branch_and_folder_follow_it() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    let ws = fx.create(&project.name, "Caf\u{e9} L\u{f6}we");

    assert_eq!(ws.name, "Cafe-Lowe");
    assert_eq!(ws.branch.as_deref(), Some("Cafe-Lowe"));
    assert_eq!(
        canonical(Path::new(&ws.path)),
        canonical(&fx.path("defaults/repo/Cafe-Lowe"))
    );
    assert!(branch_exists(&fx.repo(), "Cafe-Lowe"));
}

#[test]
fn names_with_nothing_to_keep_are_refused_before_git_runs() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let too_long = "x".repeat(65);

    for name in ["", "   ", "***", too_long.as_str()] {
        expect_code(
            fx.workspaces
                .create_workspace(create_params(&project.name, name)),
            codes::WORKSPACE_NAME_INVALID,
        );
    }
    assert!(!fx.path("defaults").exists());
    assert!(
        fx.workspaces
            .list_workspaces(None)
            .expect("list")
            .is_empty()
    );
}

#[test]
fn a_name_git_refuses_as_a_branch_is_a_name_error() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    expect_code(
        fx.workspaces
            .create_workspace(create_params(&project.name, "a.lock")),
        codes::WORKSPACE_NAME_INVALID,
    );
}

#[test]
fn an_explicit_branch_git_refuses_is_a_name_error() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let mut params = create_params(&project.name, "explicit");
    params.branch = Some("bad..branch".into());

    expect_code(
        fx.workspaces.create_workspace(params),
        codes::WORKSPACE_NAME_INVALID,
    );
}
