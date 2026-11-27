//! Initialising git in a folder project: `git init`, an empty first commit, then a git project with the workspaces root a new repository gets (decision 0115).

use std::path::Path;
use std::sync::Arc;
use std::thread;

use massorca_core_git::Git;
use massorca_core_workspace::{WorkspaceLayout, Workspaces};
use massorca_protocol::codes;
use massorca_protocol::methods::projects::ProjectKind;
use massorca_protocol::methods::workspaces::{WorkspaceKind, WorkspaceState};

use crate::fixture::{Fixture, expect_code, project_add, run_git, text};

#[test]
fn a_folder_project_becomes_a_git_project_with_a_repository_and_one_commit() {
    let fx = Fixture::new();
    let folder = fx.folder_project("notes");

    let project = fx
        .workspaces
        .init_project_git(&folder.name)
        .expect("init git");

    assert_eq!(project.kind, ProjectKind::Git);
    assert_eq!(project.path, folder.path, "the project keeps its folder");
    assert_eq!(
        project.workspaces_root,
        text(&fx.path("defaults").join("notes")),
        "the root a newly added repository of that name gets"
    );
    assert_eq!(
        fx.workspaces.show_project(&folder.name).expect("show").kind,
        ProjectKind::Git
    );
    let dir = fx.path("notes");
    assert_eq!(run_git(&dir, &["rev-list", "--count", "HEAD"]).trim(), "1");
    assert!(dir.join("notes.txt").is_file(), "the folder's files stay");
}

#[test]
fn the_workspaces_root_follows_the_flat_layout_like_a_new_repository() {
    let fx = Fixture::new();
    let folder = fx.folder_project("notes");
    let flat = fx.workspaces.with_layout(WorkspaceLayout::Flat);

    let project = flat.init_project_git(&folder.name).expect("init git");

    assert_eq!(project.workspaces_root, text(&fx.path("defaults")));
}

#[test]
fn a_workspace_created_after_the_init_is_a_worktree_of_the_first_commit() {
    let fx = Fixture::new();
    let folder = fx.folder_project("notes");
    fx.workspaces
        .init_project_git(&folder.name)
        .expect("init git");

    let created = fx.create(&folder.name, "feature");

    assert_eq!(created.kind, WorkspaceKind::Worktree);
    assert_eq!(created.state, WorkspaceState::Ready);
    assert!(created.branch.is_some(), "a new branch");
    assert!(Path::new(&created.path).is_dir());
}

#[test]
fn a_folder_that_is_already_a_repository_is_refused_and_stays_a_folder() {
    let fx = Fixture::new();
    let folder = fx.folder_project("notes");
    run_git(&fx.path("notes"), &["init", "-q"]);

    expect_code(
        fx.workspaces.init_project_git(&folder.name),
        codes::PROJECT_ALREADY_GIT,
    );

    assert_eq!(
        fx.workspaces.show_project(&folder.name).expect("show").kind,
        ProjectKind::Folder
    );
}

#[test]
fn a_folder_inside_a_repository_is_refused_as_already_git() {
    let fx = Fixture::new();
    let inner = fx.repo().join("inner");
    std::fs::create_dir_all(&inner).expect("create the folder");
    std::fs::write(inner.join("notes.txt"), "keep me").expect("write notes");
    let mut params = project_add(&inner);
    params.kind = Some(ProjectKind::Folder);
    let folder = fx
        .workspaces
        .add_project(params)
        .expect("add a folder inside the repository");

    expect_code(
        fx.workspaces.init_project_git(&folder.name),
        codes::PROJECT_ALREADY_GIT,
    );

    assert!(!inner.join(".git").exists());
}

#[test]
fn a_git_project_is_refused_as_already_git() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    expect_code(
        fx.workspaces.init_project_git(&project.name),
        codes::PROJECT_ALREADY_GIT,
    );
}

#[test]
fn an_unknown_project_is_not_found() {
    let fx = Fixture::new();

    expect_code(
        fx.workspaces.init_project_git("no-such-project"),
        codes::PROJECT_NOT_FOUND,
    );
}

#[test]
fn a_folder_whose_directory_is_gone_is_refused_as_a_missing_path() {
    let fx = Fixture::new();
    let folder = fx.folder_project("gone");
    std::fs::remove_dir_all(fx.path("gone")).expect("remove the folder");

    expect_code(
        fx.workspaces.init_project_git(&folder.name),
        codes::PATH_NOT_FOUND,
    );
}

#[test]
fn without_git_the_init_is_refused_as_unavailable_and_the_folder_stays_a_folder() {
    let fx = Fixture::new();
    let folder = fx.folder_project("notes");
    let without_git = Workspaces::new(
        fx.store.clone(),
        Git::with_binary(fx.path("no-such-git")),
        fx.path("defaults"),
    );

    expect_code(
        without_git.init_project_git(&folder.name),
        codes::GIT_UNAVAILABLE,
    );

    assert!(!fx.path("notes").join(".git").exists());
}

#[test]
fn existing_folder_workspaces_stay_listed_shown_and_removable() {
    let fx = Fixture::new();
    let folder = fx.folder_project("notes");
    let before = fx.create(&folder.name, "main");
    assert_eq!(before.kind, WorkspaceKind::Folder);

    fx.workspaces
        .init_project_git(&folder.name)
        .expect("init git");

    let shown = fx
        .workspaces
        .show_workspace("main")
        .expect("show the folder workspace");
    assert_eq!(shown.kind, WorkspaceKind::Folder);
    assert_eq!(shown.path, before.path);
    assert!(!shown.missing);
    let listed = fx
        .workspaces
        .list_workspaces(Some(&folder.name))
        .expect("list the project's workspaces");
    assert!(listed.iter().any(|workspace| workspace.id == before.id));

    fx.remove(&before.id);

    assert!(fx.path("notes").join("notes.txt").is_file());
}

#[test]
fn two_concurrent_inits_make_one_repository_and_refuse_the_other() {
    let fx = Fixture::new();
    let folder = fx.folder_project("notes");
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let api = Arc::clone(&fx.workspaces);
            let name = folder.name.clone();
            thread::spawn(move || api.init_project_git(&name))
        })
        .collect();

    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().expect("thread finished"))
        .collect();

    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    for error in results.iter().filter_map(|result| result.as_ref().err()) {
        assert_eq!(error.code(), codes::PROJECT_ALREADY_GIT);
    }
    assert_eq!(
        run_git(&fx.path("notes"), &["rev-list", "--count", "HEAD"]).trim(),
        "1"
    );
}
