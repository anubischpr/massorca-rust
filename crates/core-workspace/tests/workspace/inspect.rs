use std::fs;

use massorca_protocol::codes;
use massorca_protocol::methods::projects::ProjectKind;

use crate::fixture::{Fixture, canonical, expect_code, project_add, text};

#[test]
fn a_path_inside_a_repository_inspects_as_git_at_the_toplevel() {
    let fx = Fixture::new();
    let nested = fx.repo().join("src");
    fs::create_dir_all(&nested).expect("create nested dir");

    let inspection = fx
        .workspaces
        .inspect_project(&text(&nested))
        .expect("inspect");

    assert_eq!(inspection.kind, ProjectKind::Git);
    assert_eq!(inspection.path, text(&canonical(&fx.repo())));
    assert_eq!(inspection.registered, None);
}

#[test]
fn a_plain_directory_inspects_as_a_folder_and_add_registers_exactly_that() {
    let fx = Fixture::new();
    let notes = fx.path("notes");
    fs::create_dir_all(&notes).expect("create dir");

    let before = fx
        .workspaces
        .inspect_project(&text(&notes))
        .expect("inspect");
    assert_eq!(before.kind, ProjectKind::Folder);
    assert_eq!(before.path, text(&canonical(&notes)));
    assert_eq!(before.registered, None);

    let project = fx
        .workspaces
        .add_project(project_add(&notes))
        .expect("add project");
    assert_eq!(project.kind, before.kind);
    assert_eq!(project.path, before.path);

    let after = fx
        .workspaces
        .inspect_project(&text(&notes))
        .expect("inspect");
    assert_eq!(after.registered, Some(project.id));
}

#[test]
fn inspection_refuses_paths_with_the_codes_add_uses_and_writes_nothing() {
    let fx = Fixture::new();
    let file = fx.path("file.txt");
    fs::write(&file, "text").expect("write file");

    expect_code(
        fx.workspaces.inspect_project(&text(&fx.path("missing"))),
        codes::PATH_NOT_FOUND,
    );
    expect_code(
        fx.workspaces.inspect_project(&text(&file)),
        codes::PATH_NOT_DIRECTORY,
    );
    expect_code(
        fx.workspaces.inspect_project("relative/dir"),
        codes::REQUEST_INVALID_PARAMS,
    );
    fx.workspaces
        .inspect_project(&text(&fx.repo()))
        .expect("inspect");
    assert!(fx.workspaces.list_projects().expect("list").is_empty());
}
