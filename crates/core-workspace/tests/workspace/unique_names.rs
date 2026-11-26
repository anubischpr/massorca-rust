//! `unique_name`: a taken name, branch or folder moves a typed name on to `name-2`, `name-3`, ...

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::thread;

use massorca_core_workspace::BranchPrefix;
use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};

use crate::fixture::{
    Fixture, branch_exists, canonical, create_params, expect_code, run_git, text,
};

fn unique(project: &str, name: &str) -> WorkspaceCreateParams {
    WorkspaceCreateParams {
        unique_name: true,
        ..create_params(project, name)
    }
}

fn create_unique(fx: &Fixture, project: &str, name: &str) -> Workspace {
    fx.workspaces
        .create_workspace(unique(project, name))
        .expect("unique create")
}

fn add_origin(repo: &Path) {
    run_git(
        repo,
        &[
            "remote",
            "add",
            "origin",
            "https://example.invalid/repo.git",
        ],
    );
}

#[test]
fn a_free_name_is_used_as_typed() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    let ws = create_unique(&fx, &project.name, "feature");

    assert_eq!(ws.name, "feature");
    assert_eq!(ws.branch.as_deref(), Some("feature"));
}

#[test]
fn a_name_another_workspace_holds_moves_on_to_the_next_suffix_for_name_branch_and_folder() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    fx.create(&project.name, "feature");

    let ws = create_unique(&fx, &project.name, "feature");

    assert_eq!(ws.name, "feature-2");
    assert_eq!(ws.branch.as_deref(), Some("feature-2"));
    assert_eq!(
        canonical(Path::new(&ws.path)),
        canonical(&fx.path("defaults/repo/feature-2"))
    );
    assert!(branch_exists(&fx.repo(), "feature-2"));
}

#[test]
fn a_local_branch_without_a_workspace_moves_the_name_on() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    run_git(&fx.repo(), &["branch", "taken"]);

    let ws = create_unique(&fx, &project.name, "taken");

    assert_eq!(ws.name, "taken-2");
    assert_eq!(ws.branch.as_deref(), Some("taken-2"));
}

#[test]
fn a_branch_only_a_remote_tracks_moves_the_name_on() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    add_origin(&fx.repo());
    run_git(
        &fx.repo(),
        &["update-ref", "refs/remotes/origin/remote-only", "HEAD"],
    );

    let ws = create_unique(&fx, &project.name, "remote-only");

    assert_eq!(ws.name, "remote-only-2");
    assert!(!branch_exists(&fx.repo(), "remote-only"));
}

#[test]
fn a_branch_that_differs_only_in_case_moves_the_name_on() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    add_origin(&fx.repo());
    run_git(&fx.repo(), &["branch", "Mixed"]);
    run_git(
        &fx.repo(),
        &["update-ref", "refs/remotes/origin/Mixed-2", "HEAD"],
    );
    // Packed refs are compared as text, so a case-insensitive disk cannot answer for git.
    run_git(&fx.repo(), &["pack-refs", "--all"]);

    let ws = create_unique(&fx, &project.name, "mixed");

    assert_eq!(ws.name, "mixed-3");
}

#[test]
fn the_branch_prefix_is_part_of_the_branch_that_must_be_free() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    run_git(&fx.repo(), &["branch", "team/feature"]);
    let workspaces = fx
        .workspaces
        .with_branch_prefix(BranchPrefix::Custom("team".to_owned()));

    let ws = workspaces
        .create_workspace(unique(&project.name, "feature"))
        .expect("unique create");

    assert_eq!(ws.name, "feature-2");
    assert_eq!(ws.branch.as_deref(), Some("team/feature-2"));
}

#[test]
fn an_occupied_folder_moves_the_name_on() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let occupied = fx.path("defaults/repo/feature");
    fs::create_dir_all(&occupied).expect("create folder");
    fs::write(occupied.join("keep.txt"), "keep").expect("write file");

    let ws = create_unique(&fx, &project.name, "feature");

    assert_eq!(ws.name, "feature-2");
    assert!(occupied.join("keep.txt").is_file());
}

#[test]
fn a_taken_second_suffix_moves_on_to_the_third() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    fx.create(&project.name, "feature");
    run_git(&fx.repo(), &["branch", "feature-2"]);

    let ws = create_unique(&fx, &project.name, "feature");

    assert_eq!(ws.name, "feature-3");
}

#[test]
fn a_long_name_is_cut_so_the_suffixed_name_stays_within_64_characters() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let long = format!("{}-zz", "a".repeat(61));
    assert_eq!(long.len(), 64);
    fx.create(&project.name, &long);

    let ws = create_unique(&fx, &project.name, &long);

    // Cut to 62, the trailing `-` is dropped before the suffix.
    assert_eq!(ws.name, format!("{}-2", "a".repeat(61)));
}

#[test]
fn a_folder_project_moves_a_taken_name_on() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");
    let other = fx.path("other");
    fs::create_dir_all(&other).expect("create other");
    let mut elsewhere = create_params(&project.name, "notes");
    elsewhere.path = Some(text(&other));
    fx.workspaces
        .create_workspace(elsewhere)
        .expect("first folder workspace");

    let ws = create_unique(&fx, &project.name, "notes");

    assert_eq!(ws.name, "notes-2");
    assert_eq!(ws.branch, None);
    assert_eq!(canonical(Path::new(&ws.path)), canonical(&fx.path("notes")));
}

#[test]
fn unique_name_refuses_a_branch_an_existing_branch_or_a_path_the_caller_gives() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    let mut branch = unique(&project.name, "one");
    branch.branch = Some("explicit".into());
    expect_code(
        fx.workspaces.create_workspace(branch),
        codes::REQUEST_INVALID_PARAMS,
    );

    run_git(&fx.repo(), &["branch", "kept"]);
    let mut existing = unique(&project.name, "kept");
    existing.existing_branch = true;
    expect_code(
        fx.workspaces.create_workspace(existing),
        codes::REQUEST_INVALID_PARAMS,
    );

    let mut path = unique(&project.name, "three");
    path.path = Some(text(&fx.path("elsewhere")));
    expect_code(
        fx.workspaces.create_workspace(path),
        codes::REQUEST_INVALID_PARAMS,
    );
    assert!(
        fx.workspaces
            .list_workspaces(None)
            .expect("list")
            .is_empty()
    );
}

#[test]
fn without_unique_name_a_taken_name_branch_or_folder_is_still_refused() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    fx.create(&project.name, "named");
    let mut again = create_params(&project.name, "named");
    again.path = Some(text(&fx.path("elsewhere-named")));
    expect_code(
        fx.workspaces.create_workspace(again),
        codes::WORKSPACE_NAME_TAKEN,
    );

    run_git(&fx.repo(), &["branch", "branched"]);
    expect_code(
        fx.workspaces
            .create_workspace(create_params(&project.name, "branched")),
        codes::BRANCH_EXISTS,
    );

    let occupied = fx.path("defaults/repo/foldered");
    fs::create_dir_all(&occupied).expect("create folder");
    fs::write(occupied.join("keep.txt"), "keep").expect("write file");
    expect_code(
        fx.workspaces
            .create_workspace(create_params(&project.name, "foldered")),
        codes::WORKSPACE_PATH_TAKEN,
    );
}

#[test]
fn a_blank_name_with_unique_name_still_gets_a_generated_name() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    let created = fx
        .workspaces
        .create_workspace_once(unique(&project.name, ""))
        .expect("generated create");

    assert!(!created.workspace.name.is_empty());
}

#[test]
fn concurrent_unique_creates_of_one_name_each_take_a_different_name() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let api = Arc::clone(&fx.workspaces);
            let project = project.name.clone();
            thread::spawn(move || api.create_workspace(unique(&project, "same")))
        })
        .collect();

    let mut names: Vec<String> = handles
        .into_iter()
        .map(|handle| {
            handle
                .join()
                .expect("thread finished")
                .expect("unique create")
                .name
        })
        .collect();
    names.sort();

    assert_eq!(names, ["same", "same-2", "same-3", "same-4"]);
}
