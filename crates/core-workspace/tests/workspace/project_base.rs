//! A project's base: where its new workspaces start, and how it is set, refused and cleared (WS-04, ADR 0008).

use std::path::Path;

use massorca_protocol::codes;
use massorca_protocol::methods::projects::{Project, ProjectSetBaseParams};

use crate::fixture::{Fixture, commit_file, create_params, expect_code, run_git};

/// Adds a `develop` branch with one commit of its own, then checks `main` out again.
fn develop_branch(fx: &Fixture) {
    let repo = fx.repo();
    run_git(&repo, &["branch", "develop"]);
    run_git(&repo, &["checkout", "-q", "develop"]);
    commit_file(&repo, "develop.md", "develop\n", "develop commit");
    run_git(&repo, &["checkout", "-q", "main"]);
}

fn set_base(fx: &Fixture, project: &str, base: Option<&str>) -> Project {
    fx.workspaces
        .set_project_base(ProjectSetBaseParams {
            project: project.to_owned(),
            base: base.map(str::to_owned),
        })
        .expect("set the project base")
}

#[test]
fn a_new_workspace_starts_from_the_project_base_and_records_it() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    develop_branch(&fx);

    let set = set_base(&fx, &project.name, Some("develop"));
    assert_eq!(set.base_ref.as_deref(), Some("develop"));
    let shown = fx.workspaces.show_project(&project.name).expect("show");
    assert_eq!(shown.base_ref.as_deref(), Some("develop"));

    let created = fx.create(&project.name, "feature");

    assert_eq!(created.base_ref.as_deref(), Some("develop"));
    assert!(Path::new(&created.path).join("develop.md").is_file());
}

#[test]
fn an_explicit_base_wins_over_the_project_base() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    develop_branch(&fx);
    set_base(&fx, &project.name, Some("develop"));
    let mut params = create_params(&project.name, "hotfix");
    params.base = Some("main".to_owned());

    let created = fx
        .workspaces
        .create_workspace(params)
        .expect("create with an explicit base");

    assert_eq!(created.base_ref.as_deref(), Some("main"));
    assert!(!Path::new(&created.path).join("develop.md").exists());
}

#[test]
fn a_base_that_does_not_resolve_is_refused_and_not_stored() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    expect_code(
        fx.workspaces.set_project_base(ProjectSetBaseParams {
            project: project.name.clone(),
            base: Some("no-such-branch".to_owned()),
        }),
        codes::BASE_NOT_FOUND,
    );

    let shown = fx.workspaces.show_project(&project.name).expect("show");
    assert_eq!(shown.base_ref, None);
}

#[test]
fn a_folder_project_cannot_have_a_base() {
    let fx = Fixture::new();
    let folder = fx.folder_project("notes");

    expect_code(
        fx.workspaces.set_project_base(ProjectSetBaseParams {
            project: folder.name.clone(),
            base: Some("main".to_owned()),
        }),
        codes::REQUEST_INVALID_PARAMS,
    );
}

#[test]
fn clearing_the_project_base_restores_the_repository_default() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    develop_branch(&fx);
    set_base(&fx, &project.name, Some("develop"));

    let cleared = set_base(&fx, &project.name, None);
    assert_eq!(cleared.base_ref, None);
    let created = fx.create(&project.name, "feature");

    assert_eq!(created.base_ref.as_deref(), Some("main"));
    assert!(!Path::new(&created.path).join("develop.md").exists());
}
