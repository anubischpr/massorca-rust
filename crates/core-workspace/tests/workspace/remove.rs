//! Removing workspaces: refusals, the two halves of the removal, and resumption.

use std::fs;
use std::path::Path;

use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::WorkspaceState;

use crate::fixture::{
    Fixture, create_params, entries, expect_code, run_git,
    worktree_count,
};

fn state_of(fx: &Fixture, id: &str) -> WorkspaceState {
    fx.workspaces.show_workspace(id).expect("show").state
}

#[test]
fn removing_a_clean_worktree_deletes_its_directory_row_and_created_branch() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "feature");

    let begun = fx
        .workspaces
        .begin_remove("feature", false)
        .expect("begin removal");

    assert_eq!(begun.state, WorkspaceState::Removing);
    assert!(Path::new(&ws.path).exists());
    assert_eq!(state_of(&fx, &ws.id), WorkspaceState::Removing);

    let removed = fx
        .workspaces
        .finish_remove(&ws.id, false)
        .expect("finish removal");

    assert_eq!(removed.workspace.id, ws.id);
    assert_eq!(removed.branch_kept_reason, None);
    assert!(!Path::new(&ws.path).exists());
    assert_eq!(worktree_count(&fx.repo()), 1);
    expect_code(
        fx.workspaces.show_workspace(&ws.id),
        codes::WORKSPACE_NOT_FOUND,
    );
}

#[test]
fn a_missing_directory_is_removed_from_git_and_the_store() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ghost = fx.create(&project.name, "ghost");
    fs::remove_dir_all(&ghost.path).expect("remove directory");
    assert!(
        fx.workspaces
            .show_workspace(&ghost.id)
            .expect("show")
            .missing
    );

    let begun = fx
        .workspaces
        .begin_remove("ghost", false)
        .expect("begin removal of a missing workspace");
    let removed = fx
        .workspaces
        .finish_remove(&begun.id, false)
        .expect("finish removal");

    assert_eq!(worktree_count(&fx.repo()), 1);
    expect_code(
        fx.workspaces.show_workspace(&ghost.id),
        codes::WORKSPACE_NOT_FOUND,
    );
}

#[test]
fn a_missing_directory_is_also_removed_from_git_when_forced() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ghost = fx.create(&project.name, "forced-ghost");
    fs::remove_dir_all(&ghost.path).expect("remove directory");

    let begun = fx
        .workspaces
        .begin_remove("forced-ghost", true)
        .expect("begin with force");
    let removed = fx
        .workspaces
        .finish_remove(&begun.id, true)
        .expect("finish with force");

    assert_eq!(worktree_count(&fx.repo()), 1);
}

#[test]
fn an_existing_branch_is_never_deleted() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    run_git(&fx.repo(), &["branch", "keep-me"]);
    let mut params = create_params(&project.name, "keep-wt");
    params.branch = Some("keep-me".into());
    params.existing_branch = true;
    fx.workspaces
        .create_workspace(params)
        .expect("create on existing branch");

    let removed = fx.remove("keep-wt");

    assert_eq!(removed.branch_kept_reason, None);
}

#[test]
fn a_folder_workspace_is_forgotten_and_its_files_stay() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");
    fx.create(&project.name, "main");

    let removed = fx.remove("main");

    assert_eq!(removed.branch_kept_reason, None);
    assert!(
        fx.workspaces
            .list_workspaces(None)
            .expect("list")
            .is_empty()
    );
    assert_eq!(entries(&fx.path("notes")), ["notes.txt"]);
}

#[test]
fn an_interrupted_removal_resumes_when_removed_again() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "resume");
    let interrupted = fx
        .workspaces
        .begin_remove("resume", false)
        .expect("first half");
    assert_eq!(interrupted.state, WorkspaceState::Removing);

    let again = fx
        .workspaces
        .begin_remove("resume", false)
        .expect("begin again on a removing workspace");
    assert_eq!(again.state, WorkspaceState::Removing);
    let removed = fx
        .workspaces
        .finish_remove(&ws.id, false)
        .expect("finish the interrupted removal");

    assert!(!Path::new(&ws.path).exists());
}

#[test]
fn finish_alone_marks_and_removes_a_ready_workspace() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "direct");

    let _removed = fx
        .workspaces
        .finish_remove(&ws.id, false)
        .expect("finish without begin");

    assert!(!Path::new(&ws.path).exists());
    assert_eq!(worktree_count(&fx.repo()), 1);
}

#[test]
fn removing_an_unknown_workspace_is_not_found() {
    let fx = Fixture::new();
    fx.repo_project();

    expect_code(
        fx.workspaces.begin_remove("nothing", false),
        codes::WORKSPACE_NOT_FOUND,
    );
}

#[test]
fn finishing_an_unknown_workspace_is_not_found() {
    let fx = Fixture::new();
    fx.repo_project();

    expect_code(
        fx.workspaces
            .finish_remove("00000000-0000-0000-0000-000000000000", false),
        codes::WORKSPACE_NOT_FOUND,
    );
}

