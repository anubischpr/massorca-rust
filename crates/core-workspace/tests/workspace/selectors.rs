use std::fs;

use massorca_protocol::codes;

use crate::fixture::{Fixture, expect_code, project_add, text};

#[test]
fn a_project_resolves_by_id_then_name_then_path() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    assert_eq!(
        fx.workspaces.show_project(&project.id).expect("by id"),
        project
    );
    assert_eq!(
        fx.workspaces.show_project("repo").expect("by name"),
        project
    );
    assert_eq!(
        fx.workspaces
            .show_project(&text(&fx.repo()))
            .expect("by path"),
        project
    );
}

#[test]
fn a_project_whose_directory_is_gone_still_resolves_by_its_path() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");
    let notes = fx.path("notes");
    fs::remove_dir_all(&notes).expect("remove directory");

    let shown = fx.workspaces.show_project(&text(&notes)).expect("by path");

    assert_eq!(shown.id, project.id);
}

#[test]
fn an_unknown_project_is_not_found() {
    let fx = Fixture::new();
    fx.repo_project();

    expect_code(
        fx.workspaces.show_project("missing"),
        codes::PROJECT_NOT_FOUND,
    );
}

#[test]
fn a_workspace_name_shared_by_two_projects_is_ambiguous_but_id_and_path_are_not() {
    let fx = Fixture::new();
    let first = fx.repo_project();
    let second_repo = fx.second_repo("other");
    let second = fx
        .workspaces
        .add_project(project_add(&second_repo))
        .expect("add second project");
    let a = fx.create(&first.name, "feature");
    let b = fx.create(&second.name, "feature");

    expect_code(
        fx.workspaces.show_workspace("feature"),
        codes::SELECTOR_AMBIGUOUS,
    );
    assert_eq!(fx.workspaces.show_workspace(&a.id).expect("by id").id, a.id);
    assert_eq!(
        fx.workspaces.show_workspace(&b.path).expect("by path").id,
        b.id
    );
}

#[test]
fn a_workspace_name_used_in_one_project_resolves_by_name() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let solo = fx.create(&project.name, "solo");

    assert_eq!(
        fx.workspaces.show_workspace("solo").expect("by name").id,
        solo.id
    );
}

#[test]
fn a_missing_workspace_directory_still_resolves_by_its_path_and_shows_as_missing() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ghost = fx.create(&project.name, "ghost");
    fs::remove_dir_all(&ghost.path).expect("remove directory");

    let shown = fx.workspaces.show_workspace(&ghost.path).expect("by path");

    assert_eq!(shown.id, ghost.id);
    assert!(shown.missing);
}

#[test]
fn an_unknown_workspace_is_not_found() {
    let fx = Fixture::new();
    fx.repo_project();

    expect_code(
        fx.workspaces.show_workspace("nothing"),
        codes::WORKSPACE_NOT_FOUND,
    );
}
