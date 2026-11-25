use std::fs;
use std::path::Path;

use massorca_protocol::codes;
use massorca_protocol::methods::projects::ProjectKind;
use massorca_protocol::methods::workspaces::WorkspaceKind;

use crate::fixture::{Fixture, expect_code, project_add, run_git};

#[test]
fn list_returns_every_workspace_or_only_those_of_one_project() {
    let fx = Fixture::new();
    let repo = fx.repo_project();
    let second_repo = fx.second_repo("other");
    let other = fx
        .workspaces
        .add_project(project_add(&second_repo))
        .expect("add second project");
    let a = fx.create(&repo.name, "a1");
    let b = fx.create(&other.name, "b1");

    let all = fx.workspaces.list_workspaces(None).expect("list all");
    assert_eq!(all.len(), 2);
    assert!(all.contains(&a));
    assert!(all.contains(&b));

    let of_repo = fx
        .workspaces
        .list_workspaces(Some(&repo.id))
        .expect("list by id");
    assert_eq!(of_repo, [a]);

    let of_other = fx
        .workspaces
        .list_workspaces(Some("other"))
        .expect("list by name");
    assert_eq!(of_other, [b]);
}

#[test]
fn a_workspace_whose_directory_is_gone_is_listed_and_shown_as_missing() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "gone");
    fs::remove_dir_all(&ws.path).expect("remove directory");

    let listed = fx.workspaces.list_workspaces(None).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, ws.id);
    assert!(listed[0].missing);

    let shown = fx.workspaces.show_workspace(&ws.id).expect("show");
    assert!(shown.missing);
}

#[test]
fn a_new_project_has_no_workspaces() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    assert!(fx.workspaces.list_workspaces(None).expect("all").is_empty());
    assert!(
        fx.workspaces
            .list_workspaces(Some(&project.id))
            .expect("of project")
            .is_empty()
    );
}

#[test]
fn listing_the_workspaces_of_an_unknown_project_is_not_found() {
    let fx = Fixture::new();
    fx.repo_project();

    expect_code(
        fx.workspaces.list_workspaces(Some("nope")),
        codes::PROJECT_NOT_FOUND,
    );
}

#[test]
fn a_locked_worktree_shows_git_lock_reason_in_list_and_show() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "usb");
    run_git(
        &fx.repo(),
        &[
            "worktree",
            "lock",
            "--reason",
            "on a usb disk",
            ws.path.as_str(),
        ],
    );

    let listed = fx.workspaces.list_workspaces(None).expect("list");
    assert_eq!(listed[0].locked.as_deref(), Some("on a usb disk"));
    assert_eq!(listed[0].prunable, None);
    let shown = fx
        .workspaces
        .show_workspace_with_git_state(&ws.id)
        .expect("show");
    assert_eq!(shown.locked.as_deref(), Some("on a usb disk"));
}

#[test]
fn a_lock_without_a_reason_is_an_empty_lock_reason() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "bare");
    run_git(&fx.repo(), &["worktree", "lock", ws.path.as_str()]);

    let shown = fx
        .workspaces
        .show_workspace_with_git_state(&ws.id)
        .expect("show");
    assert_eq!(shown.locked.as_deref(), Some(""));
    let plain = fx.workspaces.show_workspace(&ws.id).expect("show");
    assert_eq!(plain.locked, None, "a plain lookup runs no git");
}

#[test]
fn a_worktree_whose_directory_is_gone_is_prunable() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "gone");
    fs::remove_dir_all(&ws.path).expect("remove directory");

    let listed = fx.workspaces.list_workspaces(None).expect("list");
    assert!(listed[0].missing);
    // Git before 2.31 prints no `prunable`; the folder's absence fills it in on every version.
    assert!(listed[0].prunable.is_some());
}

#[test]
fn a_directory_inside_a_worktree_resolves_to_that_worktree() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "deep");
    let inside = Path::new(&ws.path).join("src").join("nested");
    fs::create_dir_all(&inside).expect("create subdirectory");

    let found = fx
        .workspaces
        .current_workspace(&inside.to_string_lossy())
        .expect("the subdirectory is inside the worktree");
    assert_eq!(found.id, ws.id);
}

#[test]
fn a_directory_inside_a_folder_workspace_resolves_to_that_folder() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");
    let ws = fx.create(&project.name, "main");
    let inside = fx.path("notes").join("sub");
    fs::create_dir_all(&inside).expect("create subdirectory");

    let found = fx
        .workspaces
        .current_workspace(&inside.to_string_lossy())
        .expect("the subdirectory is inside the folder");
    assert_eq!(found.id, ws.id);
    assert_eq!(found.kind, WorkspaceKind::Folder);
}

#[test]
fn the_longest_matching_workspace_path_wins() {
    let fx = Fixture::new();
    let outer = fx.folder_project("outer");
    let outer_ws = fx.create(&outer.name, "outer");
    let inner_dir = fx.path("outer").join("inner");
    fs::create_dir_all(&inner_dir).expect("create inner folder");
    let mut inner_params = project_add(&inner_dir);
    inner_params.kind = Some(ProjectKind::Folder);
    let inner = fx
        .workspaces
        .add_project(inner_params)
        .expect("add inner folder project");
    let inner_ws = fx.create(&inner.name, "inner");

    let in_inner = fx
        .workspaces
        .current_workspace(&inner_dir.join("file").to_string_lossy())
        .expect("inside the inner folder");
    assert_eq!(in_inner.id, inner_ws.id);
    let in_outer = fx
        .workspaces
        .current_workspace(&fx.path("outer").to_string_lossy())
        .expect("inside the outer folder");
    assert_eq!(in_outer.id, outer_ws.id);
}

#[test]
fn a_sibling_whose_name_only_starts_like_a_workspace_is_not_inside_it() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");
    fx.create(&project.name, "main");
    let sibling = fx.path("notes-old");
    fs::create_dir_all(&sibling).expect("create sibling");

    expect_code(
        fx.workspaces.current_workspace(&sibling.to_string_lossy()),
        codes::WORKSPACE_NOT_FOUND,
    );
}

#[test]
fn a_directory_outside_every_workspace_is_not_found() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    fx.create(&project.name, "deep");
    let outside = fx.path("elsewhere");
    fs::create_dir_all(&outside).expect("create directory");

    expect_code(
        fx.workspaces.current_workspace(&outside.to_string_lossy()),
        codes::WORKSPACE_NOT_FOUND,
    );
}

#[test]
fn a_relative_path_is_refused_because_the_service_cannot_resolve_it() {
    let fx = Fixture::new();
    fx.repo_project();

    expect_code(
        fx.workspaces.current_workspace("relative/dir"),
        codes::REQUEST_INVALID_PARAMS,
    );
}

#[cfg(any(windows, target_os = "macos"))]
#[test]
fn a_path_matches_its_workspace_in_any_letter_case() {
    let fx = Fixture::new();
    let project = fx.folder_project("Notes");
    let ws = fx.create(&project.name, "main");
    let inside = fx.path("Notes").join("sub");
    fs::create_dir_all(&inside).expect("create subdirectory");

    let found = fx
        .workspaces
        .current_workspace(&inside.to_string_lossy().to_uppercase())
        .expect("the same folder in another case");
    assert_eq!(found.id, ws.id);
}
