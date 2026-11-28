//! The project's own checkout as its in-place workspace (decision 0112): one row, no worktree or branch made, and removal
//! that only forgets the row.

use std::fs;
use std::path::Path;

use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{
    WorkspaceCreateParams, WorkspaceKind, WorkspaceState,
};

use crate::fixture::{
    Fixture, branch_exists, canonical, create_params, entries, expect_code, run_git, text,
    worktree_count,
};

fn in_place(project: &str, name: &str) -> WorkspaceCreateParams {
    WorkspaceCreateParams {
        in_place: true,
        ..create_params(project, name)
    }
}

#[test]
fn in_place_registers_the_project_checkout_and_makes_no_worktree_or_branch() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let worktrees = worktree_count(&fx.repo());
    let files = entries(&fx.repo());

    let ws = fx
        .workspaces
        .create_workspace(in_place(&project.name, ""))
        .expect("create in place");

    assert!(ws.in_place);
    assert_eq!(ws.kind, WorkspaceKind::Worktree);
    assert_eq!(ws.name, project.name, "a blank name takes the project's");
    assert_eq!(canonical(Path::new(&ws.path)), canonical(&fx.repo()));
    assert_eq!(ws.branch.as_deref(), Some("main"));
    assert!(!ws.created_branch);
    assert!(!ws.name_generated);
    assert_eq!(ws.state, WorkspaceState::Ready);
    assert_eq!(worktree_count(&fx.repo()), worktrees);
    assert_eq!(entries(&fx.repo()), files);
    assert!(branch_exists(&fx.repo(), "main"));
}

#[test]
fn a_repeat_in_place_call_returns_the_same_row() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let first = fx
        .workspaces
        .create_workspace(in_place(&project.name, ""))
        .expect("first call");

    let again = fx
        .workspaces
        .create_workspace(in_place(&project.name, ""))
        .expect("repeat call");

    assert_eq!(again.id, first.id);
    assert_eq!(fx.workspaces.list_workspaces(None).expect("list").len(), 1);
}

#[test]
fn in_place_takes_none_of_the_branch_search_options() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let mut branch = in_place(&project.name, "");
    branch.branch = Some("topic".to_owned());
    let mut existing = in_place(&project.name, "");
    existing.existing_branch = true;
    let mut base = in_place(&project.name, "");
    base.base = Some("main".to_owned());
    let mut refresh = in_place(&project.name, "");
    refresh.refresh_base = true;
    let mut elsewhere = in_place(&project.name, "");
    elsewhere.path = Some(text(&fx.path("elsewhere")));
    let mut unique = in_place(&project.name, "");
    unique.unique_name = true;
    let mut generated = in_place(&project.name, "");
    generated.name_generated = true;

    for params in [
        branch, existing, base, refresh, elsewhere, unique, generated,
    ] {
        expect_code(
            fx.workspaces.create_workspace(params),
            codes::REQUEST_INVALID_PARAMS,
        );
    }
    assert!(
        fx.workspaces
            .list_workspaces(None)
            .expect("list")
            .is_empty()
    );
}

#[test]
fn an_in_place_name_another_workspace_holds_is_refused() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    fx.create(&project.name, "feature");

    expect_code(
        fx.workspaces
            .create_workspace(in_place(&project.name, "feature")),
        codes::WORKSPACE_NAME_TAKEN,
    );
}

#[test]
fn removing_the_in_place_workspace_only_forgets_it() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx
        .workspaces
        .create_workspace(in_place(&project.name, ""))
        .expect("create in place");
    fs::write(fx.repo().join("notes.txt"), "unsaved").expect("write untracked file");
    let worktrees = worktree_count(&fx.repo());

    let removed = fx.remove(&ws.id);

    assert!(removed.workspace.in_place);
    assert!(removed.directory_kept);
    assert!(!removed.branch_deleted);
    assert_eq!(removed.branch_kept_reason, None);
    assert!(removed.archive.is_none());
    assert!(fx.repo().join("README.md").is_file());
    assert_eq!(
        fs::read_to_string(fx.repo().join("notes.txt")).expect("read untracked file"),
        "unsaved"
    );
    assert!(branch_exists(&fx.repo(), "main"));
    assert_eq!(worktree_count(&fx.repo()), worktrees);
    expect_code(
        fx.workspaces.show_workspace(&ws.id),
        codes::WORKSPACE_NOT_FOUND,
    );
}

#[test]
fn the_in_place_branch_is_read_from_the_checkout_each_time() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx
        .workspaces
        .create_workspace(in_place(&project.name, ""))
        .expect("create in place");
    run_git(&fx.repo(), &["checkout", "-q", "-b", "topic"]);

    let shown = fx.workspaces.show_workspace(&ws.id).expect("show");
    let listed = fx.workspaces.list_workspaces(None).expect("list");

    assert_eq!(shown.branch.as_deref(), Some("topic"));
    assert_eq!(listed[0].branch.as_deref(), Some("topic"));
    assert!(!shown.created_branch);
}

#[test]
fn in_place_on_a_folder_project_registers_or_returns_the_folder_workspace() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");
    let before = entries(&fx.path("notes"));

    let ws = fx
        .workspaces
        .create_workspace(in_place(&project.name, ""))
        .expect("create in place");
    let again = fx
        .workspaces
        .create_workspace(in_place(&project.name, ""))
        .expect("repeat call");

    assert!(ws.in_place);
    assert_eq!(ws.kind, WorkspaceKind::Folder);
    assert_eq!(canonical(Path::new(&ws.path)), canonical(&fx.path("notes")));
    assert_eq!(again.id, ws.id);
    assert_eq!(entries(&fx.path("notes")), before);
}
