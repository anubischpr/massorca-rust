//! `name_generated`: set when the service chose the name, cleared when a person renames (decision 0111).

use massorca_protocol::methods::workspaces::{WorkspaceCreateParams, WorkspaceUpdateParams};

use crate::fixture::{Fixture, create_params};

#[test]
fn a_blank_name_is_a_generated_name() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    let ws = fx
        .workspaces
        .create_workspace_once(create_params(&project.name, ""))
        .expect("create")
        .workspace;

    assert!(ws.name_generated);
    assert!(fx.workspaces.show_workspace(&ws.id).unwrap().name_generated);
}

#[test]
fn a_unique_name_is_a_generated_name() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    let ws = fx
        .workspaces
        .create_workspace(WorkspaceCreateParams {
            unique_name: true,
            ..create_params(&project.name, "fix login")
        })
        .expect("create");

    assert!(ws.name_generated);
}

#[test]
fn a_typed_name_is_not_a_generated_name() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let folder = fx.folder_project("notes");

    assert!(!fx.create(&project.name, "feature").name_generated);
    assert!(!fx.create(&folder.name, "notes-ws").name_generated);
}

#[test]
fn a_person_renaming_clears_the_flag_and_a_comment_keeps_it() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx
        .workspaces
        .create_workspace_once(create_params(&project.name, ""))
        .expect("create")
        .workspace;
    let update = |name: Option<&str>, comment: Option<&str>| {
        fx.workspaces
            .update_workspace(WorkspaceUpdateParams {
                workspace: ws.id.clone(),
                name: name.map(str::to_owned),
                comment: comment.map(str::to_owned),
            })
            .expect("update")
    };

    assert!(update(None, Some("note")).name_generated);
    assert!(!update(Some("typed"), None).name_generated);
    assert!(!fx.workspaces.show_workspace(&ws.id).unwrap().name_generated);
}
