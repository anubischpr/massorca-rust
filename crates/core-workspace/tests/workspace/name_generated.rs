//! `name_generated`: set for a blank name or a client-derived one on a service-made branch, cleared when a person
//! renames (decision 0111).

use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{
    WorkspaceCreateParams, WorkspaceRenameParams, WorkspaceUpdateParams,
};

use crate::fixture::{Fixture, create_params, expect_code, run_git};

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
fn a_unique_name_alone_is_still_the_persons() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    let ws = fx
        .workspaces
        .create_workspace(WorkspaceCreateParams {
            unique_name: true,
            ..create_params(&project.name, "fix login")
        })
        .expect("create");

    assert!(!ws.name_generated);
}

#[test]
fn a_name_the_client_derived_is_a_generated_name() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    let ws = fx
        .workspaces
        .create_workspace(WorkspaceCreateParams {
            name_generated: true,
            unique_name: true,
            ..create_params(&project.name, "fix login")
        })
        .expect("create");

    assert!(ws.name_generated);
}

#[test]
fn a_named_or_existing_branch_is_never_a_generated_name() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    run_git(&fx.repo(), &["branch", "kept"]);

    let named = fx
        .workspaces
        .create_workspace_once(WorkspaceCreateParams {
            branch: Some("feat/login".to_owned()),
            ..create_params(&project.name, "")
        })
        .expect("create on a named branch")
        .workspace;
    let derived = fx
        .workspaces
        .create_workspace(WorkspaceCreateParams {
            branch: Some("feat/other".to_owned()),
            name_generated: true,
            ..create_params(&project.name, "other")
        })
        .expect("create a derived name on a named branch");
    let existing = fx
        .workspaces
        .create_workspace(WorkspaceCreateParams {
            existing_branch: true,
            name_generated: true,
            ..create_params(&project.name, "kept")
        })
        .expect("create on an existing branch");

    assert!(!named.name_generated);
    assert!(!derived.name_generated);
    assert!(!existing.name_generated);
    expect_code(
        fx.workspaces.rename_workspace(WorkspaceRenameParams {
            workspace: named.id.clone(),
            name: "fix-login".to_owned(),
            from_generated: true,
            expected_branch: None,
        }),
        codes::WORKSPACE_NAME_NOT_GENERATED,
    );
    assert_eq!(
        fx.workspaces
            .show_workspace(&named.id)
            .unwrap()
            .branch
            .as_deref(),
        Some("feat/login")
    );
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
