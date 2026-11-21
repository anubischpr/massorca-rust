use std::fs;

use massorca_protocol::codes;

use crate::fixture::{Fixture, expect_code, project_add};

#[test]
fn list_returns_every_workspace_or_only_those_of_one_project() {
    let fx = Fixture::new();
    let repo = fx.repo_project();
    let second_repo = fx.second_repo("other");
    let other = fx
        .workspaces
        .add_project(project_add(&second_repo))
        .expect("add second project");
    let a = fx.create(&repo.name, "a1");
    let b = fx.create(&other.name, "b1");

    let all = fx.workspaces.list_workspaces(None).expect("list all");
    assert_eq!(all.len(), 2);
    assert!(all.contains(&a));
    assert!(all.contains(&b));

    let of_repo = fx
        .workspaces
        .list_workspaces(Some(&repo.id))
        .expect("list by id");
    assert_eq!(of_repo, [a]);

    let of_other = fx
        .workspaces
        .list_workspaces(Some("other"))
        .expect("list by name");
    assert_eq!(of_other, [b]);
}

#[test]
fn a_workspace_whose_directory_is_gone_is_listed_and_shown_as_missing() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "gone");
    fs::remove_dir_all(&ws.path).expect("remove directory");

    let listed = fx.workspaces.list_workspaces(None).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, ws.id);
    assert!(listed[0].missing);

    let shown = fx.workspaces.show_workspace(&ws.id).expect("show");
    assert!(shown.missing);
}

#[test]
fn a_new_project_has_no_workspaces() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    assert!(fx.workspaces.list_workspaces(None).expect("all").is_empty());
    assert!(
        fx.workspaces
            .list_workspaces(Some(&project.id))
            .expect("of project")
            .is_empty()
    );
}

#[test]
fn listing_the_workspaces_of_an_unknown_project_is_not_found() {
    let fx = Fixture::new();
    fx.repo_project();

    expect_code(
        fx.workspaces.list_workspaces(Some("nope")),
        codes::PROJECT_NOT_FOUND,
    );
}
