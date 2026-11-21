//! Registering a folder workspace in a folder project.

use std::fs;
use std::path::Path;

use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{WorkspaceKind, WorkspaceState};

use crate::fixture::{Fixture, canonical, create_params, entries, expect_code, text};

#[test]
fn a_folder_workspace_registers_the_project_directory_and_creates_nothing() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");
    let before = entries(&fx.path("notes"));

    let ws = fx.create(&project.name, "main");

    assert_eq!(ws.kind, WorkspaceKind::Folder);
    assert_eq!(ws.state, WorkspaceState::Ready);
    assert_eq!(ws.branch, None);
    assert_eq!(ws.base_ref, None);
    assert!(!ws.created_branch);
    assert!(!ws.missing);
    assert_eq!(canonical(Path::new(&ws.path)), canonical(&fx.path("notes")));
    assert_eq!(entries(&fx.path("notes")), before);
}

#[test]
fn a_folder_workspace_can_point_at_another_existing_directory() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");
    let other = fx.path("other");
    fs::create_dir_all(&other).expect("create other");
    let mut params = create_params(&project.name, "other");
    params.path = Some(text(&other));

    let ws = fx
        .workspaces
        .create_workspace(params)
        .expect("create workspace");

    assert_eq!(canonical(Path::new(&ws.path)), canonical(&other));
}

#[test]
fn a_folder_workspace_path_must_exist() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");
    let mut params = create_params(&project.name, "nowhere");
    params.path = Some(text(&fx.path("nowhere")));

    expect_code(
        fx.workspaces.create_workspace(params),
        codes::PATH_NOT_FOUND,
    );
}

#[test]
fn a_folder_workspace_path_must_be_a_directory() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");
    let file = fx.path("plain.txt");
    fs::write(&file, "text").expect("write file");
    let mut params = create_params(&project.name, "file");
    params.path = Some(text(&file));

    expect_code(
        fx.workspaces.create_workspace(params),
        codes::PATH_NOT_DIRECTORY,
    );
}

#[test]
fn a_folder_workspace_path_is_unique_among_workspaces() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");
    fx.create(&project.name, "one");

    expect_code(
        fx.workspaces
            .create_workspace(create_params(&project.name, "two")),
        codes::WORKSPACE_PATH_TAKEN,
    );
}

#[test]
fn branch_base_and_existing_branch_do_not_apply_to_a_folder() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");

    let mut with_branch = create_params(&project.name, "with-branch");
    with_branch.branch = Some("feature".into());
    expect_code(
        fx.workspaces.create_workspace(with_branch),
        codes::REQUEST_INVALID_PARAMS,
    );

    let mut with_base = create_params(&project.name, "with-base");
    with_base.base = Some("HEAD".into());
    expect_code(
        fx.workspaces.create_workspace(with_base),
        codes::REQUEST_INVALID_PARAMS,
    );

    let mut existing = create_params(&project.name, "existing");
    existing.existing_branch = true;
    expect_code(
        fx.workspaces.create_workspace(existing),
        codes::REQUEST_INVALID_PARAMS,
    );
}

#[test]
fn a_folder_workspace_name_follows_the_same_rules() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");

    expect_code(
        fx.workspaces
            .create_workspace(create_params(&project.name, "bad name")),
        codes::WORKSPACE_NAME_INVALID,
    );
}
