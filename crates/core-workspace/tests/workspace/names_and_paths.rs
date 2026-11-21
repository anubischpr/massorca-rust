//! Names and paths through the whole domain: create, list, show and remove (ADR 0008).

use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::WorkspaceState;

use crate::fixture::{
    Fixture, branch_exists, canonical, create_params, expect_code, project_add, text,
    worktree_count,
};

#[test]
fn a_workspace_under_a_root_with_spaces_and_non_ascii_round_trips_through_every_operation() {
    let fx = Fixture::new();
    let root = fx.path("ws root \u{e9}t\u{e9}");
    let mut params = project_add(&fx.repo());
    params.workspaces_root = Some(text(&root));
    let project = fx.workspaces.add_project(params).expect("add project");

    let created = fx.create(&project.name, "cafe-1");
    let expected = root.join("cafe-1");
    assert_eq!(created.path, text(&canonical(&expected)));
    assert_eq!(created.state, WorkspaceState::Ready);
    assert!(!created.missing);

    let listed = fx
        .workspaces
        .list_workspaces(Some(&project.name))
        .expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].path, created.path);

    let by_path = fx
        .workspaces
        .show_workspace(&created.path)
        .expect("show by path");
    assert_eq!(by_path.id, created.id);
    let by_name = fx
        .workspaces
        .show_workspace("cafe-1")
        .expect("show by name");
    assert_eq!(by_name.id, created.id);

    let removed = fx.remove(&created.path);
    assert!(removed.branch_deleted);
    assert!(!expected.exists());
    assert!(!branch_exists(&fx.repo(), "cafe-1"));
    assert!(
        fx.workspaces
            .list_workspaces(None)
            .expect("list")
            .is_empty()
    );
}

#[test]
fn an_explicit_path_with_spaces_and_non_ascii_is_used_and_removed_by_its_path() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let target = fx.path("my dir/\u{fc}ber");
    let mut params = create_params(&project.name, "uber");
    params.path = Some(text(&target));

    let created = fx
        .workspaces
        .create_workspace(params)
        .expect("create at an explicit path");

    assert_eq!(created.path, text(&canonical(&target)));
    assert!(target.join("README.md").is_file());
    fx.remove(&created.path);
    assert!(!target.exists());
    assert_eq!(worktree_count(&fx.repo()), 1);
}

#[test]
fn a_workspace_named_after_a_windows_device_is_refused_before_anything_is_created() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    for name in ["CON", "nul", "Com1", "AUX.txt", "lpt9.log"] {
        expect_code(
            fx.workspaces
                .create_workspace(create_params(&project.name, name)),
            codes::WORKSPACE_NAME_INVALID,
        );
    }

    assert!(!fx.path("defaults").exists());
    assert_eq!(worktree_count(&fx.repo()), 1);
    assert!(!branch_exists(&fx.repo(), "CON"));
}
