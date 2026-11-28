//! Where a project's default workspaces go: in a folder of the project's own under the default root
//! (nested), or directly in the default root (flat). A project keeps the root it was added with (ADR 0008).
//! The fixture pins nested; `in_project_layout` covers the default.

use std::path::{Path, PathBuf};

use massorca_core_workspace::WorkspaceLayout;
use massorca_protocol::codes;

use crate::fixture::{Fixture, canonical, create_params, expect_code, project_add, text};

#[test]
fn nested_puts_a_workspace_in_its_projects_folder() {
    let fx = Fixture::new();

    let project = fx.repo_project();
    let workspace = fx.create(&project.id, "feature");

    assert_eq!(
        project.workspaces_root,
        text(&fx.path("defaults").join("repo"))
    );
    assert_eq!(
        canonical(Path::new(&workspace.path)),
        canonical(&fx.path("defaults/repo/feature"))
    );
}

#[test]
fn flat_puts_a_new_projects_workspaces_directly_in_the_default_root() {
    let fx = Fixture::new();
    let flat = fx.workspaces.with_layout(WorkspaceLayout::Flat);

    let project = flat
        .add_project(project_add(&fx.repo()))
        .expect("add project");
    let workspace = flat
        .create_workspace(create_params(&project.id, "feature"))
        .expect("create workspace");

    assert_eq!(project.workspaces_root, text(&fx.path("defaults")));
    assert_eq!(
        canonical(Path::new(&workspace.path)),
        canonical(&fx.path("defaults/feature"))
    );
}

#[test]
fn flat_projects_with_one_workspace_name_collide_and_the_second_is_refused_by_path() {
    let fx = Fixture::new();
    let flat = fx.workspaces.with_layout(WorkspaceLayout::Flat);
    let repo = flat
        .add_project(project_add(&fx.repo()))
        .expect("add repository project");
    let other = flat
        .add_project(project_add(&fx.second_repo("other")))
        .expect("add second project");
    flat.create_workspace(create_params(&repo.id, "feature"))
        .expect("first feature");

    expect_code(
        flat.create_workspace(create_params(&other.id, "feature")),
        codes::WORKSPACE_PATH_TAKEN,
    );
}

#[test]
fn a_project_added_before_the_layout_changes_keeps_its_folder() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let flat = fx.workspaces.with_layout(WorkspaceLayout::Flat);

    let workspace = flat
        .create_workspace(create_params(&project.id, "feature"))
        .expect("create workspace");

    assert_eq!(
        canonical(Path::new(&workspace.path)),
        canonical(&fx.path("defaults/repo/feature"))
    );
}

#[test]
fn an_explicit_workspaces_root_is_kept_under_either_layout() {
    let fx = Fixture::new();
    let flat = fx.workspaces.with_layout(WorkspaceLayout::Flat);
    let mut params = project_add(&fx.repo());
    params.workspaces_root = Some(text(&fx.path("chosen")));

    let project = flat.add_project(params).expect("add project");

    assert_eq!(project.workspaces_root, text(&fx.path("chosen")));
}

#[test]
fn a_flat_default_root_that_is_not_absolute_is_refused_when_a_project_would_take_it() {
    let fx = Fixture::new();
    let flat = fx
        .workspaces
        .with_layout(WorkspaceLayout::Flat)
        .with_default_root(PathBuf::from("relative/root"));

    expect_code(
        flat.add_project(project_add(&fx.repo())),
        codes::REQUEST_INVALID_PARAMS,
    );
    assert!(fx.workspaces.list_projects().expect("list").is_empty());
}
