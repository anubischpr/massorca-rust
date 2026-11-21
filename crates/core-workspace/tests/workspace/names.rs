use massorca_protocol::codes;

use crate::fixture::{Fixture, create_params, expect_code};

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
fn names_outside_the_rules_are_refused_before_git_runs() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let too_long = "x".repeat(65);

    for name in [
        "",
        "-leading",
        ".leading",
        "has space",
        "slash/inside",
        "back\\slash",
        "unicode\u{e9}",
        "trailing.",
        too_long.as_str(),
    ] {
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
