//! Creating a worktree workspace in a git project.

use std::fs;
use std::path::Path;

use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{WorkspaceKind, WorkspaceState};

use crate::fixture::{
    Fixture, branch_exists, canonical, create_params, entries, expect_code, project_add, run_git,
    text, worktree_count,
};

#[test]
fn by_default_the_branch_takes_the_name_and_the_directory_sits_under_the_workspaces_root() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    let ws = fx.create(&project.name, "feature");

    assert_eq!(ws.kind, WorkspaceKind::Worktree);
    assert_eq!(ws.state, WorkspaceState::Ready);
    assert_eq!(ws.project_id, project.id);
    assert_eq!(ws.branch.as_deref(), Some("feature"));
    assert_eq!(ws.base_ref.as_deref(), Some("HEAD"));
    assert!(ws.created_branch);
    assert!(!ws.missing);
    assert_eq!(
        canonical(Path::new(&ws.path)),
        canonical(&fx.path("defaults/repo/feature"))
    );
    assert!(branch_exists(&fx.repo(), "feature"));
}

#[test]
fn a_branch_a_base_and_a_path_can_each_be_given() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    run_git(&fx.repo(), &["branch", "release"]);
    let target = fx.path("elsewhere/checkout");
    let mut params = create_params(&project.name, "checkout");
    params.branch = Some("feat/checkout".into());
    params.base = Some("release".into());
    params.path = Some(text(&target));

    let ws = fx
        .workspaces
        .create_workspace(params)
        .expect("create workspace");

    assert_eq!(ws.branch.as_deref(), Some("feat/checkout"));
    assert_eq!(ws.base_ref.as_deref(), Some("release"));
    assert_eq!(canonical(Path::new(&ws.path)), canonical(&target));
    assert!(branch_exists(&fx.repo(), "feat/checkout"));
}

#[test]
fn a_repeated_request_id_returns_the_first_workspace_and_creates_nothing_else() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let mut first = create_params(&project.name, "once");
    first.request_id = Some("req-1".into());
    let created = fx
        .workspaces
        .create_workspace(first)
        .expect("first request");
    let mut again = create_params(&project.name, "twice");
    again.request_id = Some("req-1".into());

    let repeated = fx
        .workspaces
        .create_workspace(again)
        .expect("repeated request");

    assert_eq!(repeated, created);
    assert_eq!(fx.workspaces.list_workspaces(None).expect("list").len(), 1);
    assert!(!fx.path("defaults/repo/twice").exists());
}

#[test]
fn an_empty_request_id_is_refused() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let mut params = create_params(&project.name, "blank-id");
    params.request_id = Some(String::new());

    expect_code(
        fx.workspaces.create_workspace(params),
        codes::REQUEST_INVALID_PARAMS,
    );
}

#[test]
fn a_name_is_unique_within_its_project() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    fx.create(&project.name, "dup");
    let mut params = create_params(&project.name, "dup");
    params.path = Some(text(&fx.path("elsewhere-dup")));

    expect_code(
        fx.workspaces.create_workspace(params),
        codes::WORKSPACE_NAME_TAKEN,
    );
    assert!(!fx.path("elsewhere-dup").exists());
}

#[test]
fn an_existing_local_branch_blocks_a_new_branch_of_the_same_name() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    run_git(&fx.repo(), &["branch", "taken"]);

    expect_code(
        fx.workspaces
            .create_workspace(create_params(&project.name, "taken")),
        codes::BRANCH_EXISTS,
    );
    assert!(!fx.path("defaults/repo/taken").exists());
}

#[test]
fn a_remote_tracking_branch_of_the_same_name_blocks_a_new_branch() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    // Only refs under a configured remote count as remote-tracking branches.
    run_git(
        &fx.repo(),
        &[
            "remote",
            "add",
            "origin",
            "https://example.invalid/repo.git",
        ],
    );
    run_git(
        &fx.repo(),
        &["update-ref", "refs/remotes/origin/remote-only", "HEAD"],
    );

    expect_code(
        fx.workspaces
            .create_workspace(create_params(&project.name, "remote-only")),
        codes::BRANCH_EXISTS,
    );
    assert!(!branch_exists(&fx.repo(), "remote-only"));
}

#[test]
fn an_existing_branch_is_checked_out_when_asked_for() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    run_git(&fx.repo(), &["branch", "kept"]);
    let mut params = create_params(&project.name, "kept-wt");
    params.branch = Some("kept".into());
    params.existing_branch = true;

    let ws = fx
        .workspaces
        .create_workspace(params)
        .expect("create workspace");

    assert_eq!(ws.branch.as_deref(), Some("kept"));
    assert_eq!(ws.base_ref, None);
    assert!(!ws.created_branch);
    assert!(Path::new(&ws.path).join("README.md").exists());
}

#[test]
fn an_existing_branch_that_is_missing_is_not_found() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let mut params = create_params(&project.name, "nope-wt");
    params.branch = Some("nope".into());
    params.existing_branch = true;

    expect_code(
        fx.workspaces.create_workspace(params),
        codes::BRANCH_NOT_FOUND,
    );
}

#[test]
fn an_existing_branch_cannot_also_be_given_a_base() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    run_git(&fx.repo(), &["branch", "kept2"]);
    let mut params = create_params(&project.name, "kept2-wt");
    params.branch = Some("kept2".into());
    params.existing_branch = true;
    params.base = Some("HEAD".into());

    expect_code(
        fx.workspaces.create_workspace(params),
        codes::REQUEST_INVALID_PARAMS,
    );
}

#[test]
fn a_base_that_does_not_resolve_is_not_found() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let mut params = create_params(&project.name, "based");
    params.base = Some("no-such-ref".into());

    expect_code(
        fx.workspaces.create_workspace(params),
        codes::BASE_NOT_FOUND,
    );
    assert!(!branch_exists(&fx.repo(), "based"));
}

#[test]
fn a_repository_without_commits_has_no_head_to_branch_from() {
    let fx = Fixture::new();
    let empty = fx.path("empty");
    fs::create_dir_all(&empty).expect("create dir");
    run_git(&empty, &["init", "-q"]);
    let project = fx
        .workspaces
        .add_project(project_add(&empty))
        .expect("add empty repository");

    expect_code(
        fx.workspaces
            .create_workspace(create_params(&project.name, "first")),
        codes::BASE_NOT_FOUND,
    );
}

#[test]
fn a_non_empty_directory_at_the_path_is_taken_and_nothing_is_created() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let target = fx.path("occupied");
    fs::create_dir_all(&target).expect("create dir");
    fs::write(target.join("notes.txt"), "mine").expect("write file");
    let mut params = create_params(&project.name, "occupied-wt");
    params.path = Some(text(&target));

    expect_code(
        fx.workspaces.create_workspace(params),
        codes::WORKSPACE_PATH_TAKEN,
    );
    assert!(!branch_exists(&fx.repo(), "occupied-wt"));
    assert_eq!(entries(&target), ["notes.txt"]);
}

#[test]
fn an_empty_directory_at_the_path_is_used() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let target = fx.path("empty-dir");
    fs::create_dir_all(&target).expect("create dir");
    let mut params = create_params(&project.name, "empty-dir-wt");
    params.path = Some(text(&target));

    let ws = fx
        .workspaces
        .create_workspace(params)
        .expect("empty directory is usable");

    assert_eq!(canonical(Path::new(&ws.path)), canonical(&target));
}

#[test]
fn a_workspace_at_another_workspaces_path_is_taken() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let first = fx.create(&project.name, "first");
    let mut params = create_params(&project.name, "second");
    params.path = Some(first.path.clone());

    expect_code(
        fx.workspaces.create_workspace(params),
        codes::WORKSPACE_PATH_TAKEN,
    );
}

#[test]
fn a_relative_path_is_refused() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let mut params = create_params(&project.name, "relative");
    params.path = Some("relative/place".into());

    expect_code(
        fx.workspaces.create_workspace(params),
        codes::REQUEST_INVALID_PARAMS,
    );
}

#[test]
fn an_unknown_project_is_not_found() {
    let fx = Fixture::new();

    expect_code(
        fx.workspaces
            .create_workspace(create_params("no-such-project", "x")),
        codes::PROJECT_NOT_FOUND,
    );
}

#[test]
fn the_project_can_be_named_by_id_and_a_created_workspace_lists_and_shows_the_same() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    let created = fx
        .workspaces
        .create_workspace(create_params(&project.id, "by-id"))
        .expect("create by project id");

    assert_eq!(
        fx.workspaces.show_workspace(&created.id).expect("show"),
        created
    );
    assert_eq!(
        fx.workspaces.list_workspaces(None).expect("list"),
        [created]
    );
}

#[test]
fn a_project_whose_workspaces_root_is_relative_is_refused_before_git_runs() {
    let fx = Fixture::new();
    fx.store
        .insert_project(&massorca_core_store::ProjectRow {
            id: massorca_core_store::new_id(),
            kind: massorca_core_store::ProjectKind::Git,
            path: text(&canonical(&fx.repo())),
            path_key: "relative-root-project".to_owned(),
            name: "odd".to_owned(),
            workspaces_root: "relative/workspaces".to_owned(),
            added_at: massorca_core_store::now_ms(),
        })
        .expect("insert a project with a relative root");

    expect_code(
        fx.workspaces
            .create_workspace(create_params("odd", "worker")),
        codes::REQUEST_INVALID_PARAMS,
    );
    assert_eq!(worktree_count(&fx.repo()), 1, "git created nothing");
}
