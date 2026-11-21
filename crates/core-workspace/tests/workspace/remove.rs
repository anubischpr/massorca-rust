//! Removing workspaces: refusals, the two halves of the removal, and resumption.

use std::fs;
use std::path::Path;

use massorca_core_store::{
    WorkspaceKind as StoreWorkspaceKind, WorkspaceRow, WorkspaceState as StoreWorkspaceState,
    new_id, now_ms,
};
use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::WorkspaceState;

use crate::fixture::{
    Fixture, branch_exists, commit_file, create_params, entries, expect_code, run_git,
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
    assert!(removed.branch_deleted);
    assert_eq!(removed.branch_kept_reason, None);
    assert!(!Path::new(&ws.path).exists());
    assert!(!branch_exists(&fx.repo(), "feature"));
    assert_eq!(worktree_count(&fx.repo()), 1);
    expect_code(
        fx.workspaces.show_workspace(&ws.id),
        codes::WORKSPACE_NOT_FOUND,
    );
}

#[test]
fn a_dirty_worktree_is_refused_and_returns_to_ready() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "dirty");
    fs::write(Path::new(&ws.path).join("scratch.txt"), "unsaved").expect("write file");

    expect_code(
        fx.workspaces.begin_remove("dirty", false),
        codes::WORKSPACE_DIRTY,
    );

    assert_eq!(state_of(&fx, &ws.id), WorkspaceState::Ready);
    assert!(Path::new(&ws.path).join("scratch.txt").exists());
}

#[test]
fn force_removes_a_dirty_worktree_after_the_refusal() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "forced");
    fs::write(Path::new(&ws.path).join("scratch.txt"), "unsaved").expect("write file");
    expect_code(
        fx.workspaces.begin_remove("forced", false),
        codes::WORKSPACE_DIRTY,
    );

    let begun = fx
        .workspaces
        .begin_remove("forced", true)
        .expect("begin with force");
    let removed = fx
        .workspaces
        .finish_remove(&begun.id, true)
        .expect("finish with force");

    assert!(removed.branch_deleted);
    assert!(!Path::new(&ws.path).exists());
    assert!(!branch_exists(&fx.repo(), "forced"));
}

#[test]
fn a_locked_worktree_is_refused_even_with_force() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "locked");
    run_git(&fx.repo(), &["worktree", "lock", ws.path.as_str()]);

    expect_code(
        fx.workspaces.begin_remove("locked", true),
        codes::WORKSPACE_LOCKED,
    );

    assert_eq!(state_of(&fx, &ws.id), WorkspaceState::Ready);
    assert!(Path::new(&ws.path).exists());
    run_git(&fx.repo(), &["worktree", "unlock", ws.path.as_str()]);
    fx.remove("locked");
    assert!(!Path::new(&ws.path).exists());
}

#[test]
fn the_main_worktree_is_never_removed() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let row = WorkspaceRow {
        id: new_id(),
        project_id: project.id.clone(),
        kind: StoreWorkspaceKind::Worktree,
        name: "main-row".into(),
        path: project.path.clone(),
        path_key: "recomputed-from-path".into(),
        branch: Some("main".into()),
        base_ref: None,
        created_branch: false,
        state: StoreWorkspaceState::Ready,
        request_id: None,
        created_at: now_ms(),
    };
    fx.store.insert_workspace(&row).expect("insert main row");

    expect_code(
        fx.workspaces.begin_remove("main-row", true),
        codes::WORKSPACE_MAIN_WORKTREE,
    );

    assert_eq!(state_of(&fx, &row.id), WorkspaceState::Ready);
    assert!(fx.repo().join("README.md").exists());
    assert_eq!(worktree_count(&fx.repo()), 1);
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

    assert!(removed.branch_deleted);
    assert_eq!(worktree_count(&fx.repo()), 1);
    assert!(!branch_exists(&fx.repo(), "ghost"));
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

    assert!(removed.branch_deleted);
    assert_eq!(worktree_count(&fx.repo()), 1);
    assert!(!branch_exists(&fx.repo(), "forced-ghost"));
}

#[test]
fn a_branch_with_commits_outside_its_base_is_kept_as_not_merged() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "unmerged");
    commit_file(
        Path::new(&ws.path),
        "work.txt",
        "work\n",
        "work on unmerged",
    );

    let removed = fx.remove("unmerged");

    assert!(!removed.branch_deleted);
    assert_eq!(removed.branch_kept_reason.as_deref(), Some("not merged"));
    assert!(branch_exists(&fx.repo(), "unmerged"));
    assert!(!Path::new(&ws.path).exists());
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

    assert!(!removed.branch_deleted);
    assert_eq!(removed.branch_kept_reason, None);
    assert!(branch_exists(&fx.repo(), "keep-me"));
}

#[test]
fn a_folder_workspace_is_forgotten_and_its_files_stay() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");
    fx.create(&project.name, "main");

    let removed = fx.remove("main");

    assert!(!removed.branch_deleted);
    assert_eq!(removed.branch_kept_reason, None);
    assert!(removed.directory_kept);
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

    assert!(removed.branch_deleted);
    assert!(!Path::new(&ws.path).exists());
}

#[test]
fn finish_alone_marks_and_removes_a_ready_workspace() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "direct");

    let removed = fx
        .workspaces
        .finish_remove(&ws.id, false)
        .expect("finish without begin");

    assert!(removed.branch_deleted);
    assert!(!Path::new(&ws.path).exists());
    assert_eq!(worktree_count(&fx.repo()), 1);
}

#[test]
fn dirt_that_appears_after_begin_is_refused_by_finish_and_returns_to_ready() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "late-dirt");
    let begun = fx
        .workspaces
        .begin_remove("late-dirt", false)
        .expect("begin on a clean worktree");
    fs::write(Path::new(&ws.path).join("late.txt"), "typed after begin").expect("write file");

    expect_code(
        fx.workspaces.finish_remove(&begun.id, false),
        codes::WORKSPACE_DIRTY,
    );

    assert_eq!(state_of(&fx, &ws.id), WorkspaceState::Ready);
    assert!(Path::new(&ws.path).join("late.txt").exists());
}

#[test]
fn a_refused_retry_of_an_interrupted_removal_returns_to_ready() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "retry");
    fx.workspaces
        .begin_remove("retry", false)
        .expect("first half");
    fs::write(Path::new(&ws.path).join("new.txt"), "dirt").expect("write file");

    expect_code(
        fx.workspaces.begin_remove("retry", false),
        codes::WORKSPACE_DIRTY,
    );

    assert_eq!(state_of(&fx, &ws.id), WorkspaceState::Ready);
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

#[test]
fn an_unregistered_directory_is_forgotten_and_kept_untouched() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let stray = fx.create(&project.name, "stray");
    // Git forgets the worktree but the directory is left behind, with work in it.
    run_git(&fx.repo(), &["worktree", "remove", "--force", &stray.path]);
    fs::create_dir_all(&stray.path).expect("recreate directory");
    fs::write(Path::new(&stray.path).join("notes.txt"), "keep me").expect("write notes");

    let removed = fx.remove("stray");

    assert!(removed.directory_kept);
    assert_eq!(entries(Path::new(&stray.path)), ["notes.txt"]);
    expect_code(
        fx.workspaces.show_workspace(&stray.id),
        codes::WORKSPACE_NOT_FOUND,
    );
}

#[test]
fn an_unregistered_directory_with_changes_is_kept_without_force() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let stray = fx.create(&project.name, "dirty-stray");
    run_git(&fx.repo(), &["worktree", "remove", "--force", &stray.path]);
    fs::create_dir_all(&stray.path).expect("recreate directory");
    fs::write(Path::new(&stray.path).join("untracked.txt"), "work").expect("write file");

    let removed = fx.remove("dirty-stray");

    assert!(removed.directory_kept);
    assert!(Path::new(&stray.path).join("untracked.txt").exists());
}

#[test]
fn a_removed_worktree_reports_that_no_directory_was_kept() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    fx.create(&project.name, "gone-clean");

    let removed = fx.remove("gone-clean");

    assert!(!removed.directory_kept);
}
