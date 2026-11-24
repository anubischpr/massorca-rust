//! A blank name asks for a generated one: a creature name for the branch and the folder (ADR 0008).

use std::path::Path;

use massorca_protocol::methods::workspaces::{WorkspaceKind, WorkspaceState};

use crate::fixture::{Fixture, branch_exists, canonical, create_params};

#[test]
fn a_blank_name_gets_a_creature_name_that_is_also_the_branch_and_the_folder() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    let ws = fx
        .workspaces
        .create_workspace_with_generated_name(create_params(&project.name, ""))
        .expect("create with a generated name");

    assert_eq!(ws.kind, WorkspaceKind::Worktree);
    assert_eq!(ws.state, WorkspaceState::Ready);
    assert!(
        ws.name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-' || byte.is_ascii_digit()),
        "a lowercase creature name: {}",
        ws.name
    );
    assert_eq!(ws.branch.as_deref(), Some(ws.name.as_str()));
    assert_eq!(
        canonical(Path::new(&ws.path)),
        canonical(&fx.path(&format!("defaults/repo/{}", ws.name)))
    );
    assert!(branch_exists(&fx.repo(), &ws.name));
}

#[test]
fn a_second_generated_name_is_different_from_the_first() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    let first = fx
        .workspaces
        .create_workspace_with_generated_name(create_params(&project.name, ""))
        .expect("first generated name");
    let second = fx
        .workspaces
        .create_workspace_with_generated_name(create_params(&project.name, "   "))
        .expect("second generated name");

    assert_ne!(first.name, second.name);
    assert_eq!(
        fx.workspaces
            .list_workspaces(Some(&project.name))
            .expect("list")
            .len(),
        2
    );
}
