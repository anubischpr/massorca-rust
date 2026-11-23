//! Refreshing a new workspace's base from its remote before the create (decisions 0008 and 0013).

use massorca_protocol::methods::workspaces::BaseRefresh;
use std::path::{Path, PathBuf};

use massorca_protocol::codes;
use massorca_protocol::methods::projects::{Project, ProjectSetBaseParams};

use crate::fixture::{
    Fixture, commit_file, create_params, expect_code, project_add, run_git, text,
};

/// Makes `<root>/remote.git` from the fixture repository and registers `<root>/clone`, a clone of it,
/// as a git project. Returns the bare remote's path and the project.
fn clone_project(fx: &Fixture) -> (PathBuf, Project) {
    let bare = fx.path("remote.git");
    let (repo_text, bare_text) = (text(&fx.repo()), text(&bare));
    run_git(
        &fx.path(""),
        &["clone", "-q", "--bare", &repo_text, &bare_text],
    );
    let clone = fx.path("clone");
    let clone_text = text(&clone);
    run_git(&fx.path(""), &["clone", "-q", &bare_text, &clone_text]);
    let project = fx
        .workspaces
        .add_project(project_add(&clone))
        .expect("add the clone as a project");
    (bare, project)
}

/// Pushes one commit to `main` of `bare` from a second clone, so the remote is ahead of every local copy.
fn advance_remote(fx: &Fixture, bare: &Path, file: &str) {
    let other = fx.path(&format!("other-{file}"));
    let (bare_text, other_text) = (text(bare), text(&other));
    run_git(&fx.path(""), &["clone", "-q", &bare_text, &other_text]);
    commit_file(&other, file, "remote\n", &format!("remote adds {file}"));
    run_git(&other, &["push", "-q", "origin", "main"]);
}

fn rev(dir: &Path, revision: &str) -> String {
    run_git(dir, &["rev-parse", revision]).trim().to_owned()
}

#[test]
fn a_refresh_starts_from_the_remote_branch_and_a_plain_create_from_the_local_copy() {
    let fx = Fixture::new();
    let (bare, project) = clone_project(&fx);
    let clone = fx.path("clone");
    let local = rev(&clone, "HEAD");
    advance_remote(&fx, &bare, "first.txt");
    let remote = rev(&bare, "refs/heads/main");

    let plain = fx.create(&project.name, "plain");
    assert_eq!(rev(Path::new(&plain.path), "HEAD"), local);

    let mut params = create_params(&project.name, "fresh");
    params.refresh_base = true;
    let fresh = fx
        .workspaces
        .create_workspace(params)
        .expect("refreshing create");
    assert_eq!(rev(Path::new(&fresh.path), "HEAD"), remote);
    assert_eq!(rev(&clone, "refs/remotes/origin/main"), remote);
    assert_eq!(
        fresh.base_refresh,
        Some(BaseRefresh::Fetched { moved: true })
    );
    assert_eq!(plain.base_refresh, None, "only a refresh reports one");
}

#[test]
fn an_unreachable_remote_still_creates_from_the_local_copy() {
    let fx = Fixture::new();
    let (bare, project) = clone_project(&fx);
    let clone = fx.path("clone");
    let local = rev(&clone, "HEAD");
    advance_remote(&fx, &bare, "first.txt");
    let gone = text(&fx.path("gone.git"));
    run_git(&clone, &["remote", "set-url", "origin", &gone]);

    let mut params = create_params(&project.name, "offline");
    params.refresh_base = true;
    let created = fx
        .workspaces
        .create_workspace(params)
        .expect("the create goes on");

    assert_eq!(rev(Path::new(&created.path), "HEAD"), local);
    assert_eq!(created.base_ref.as_deref(), Some("origin/main"));
    assert!(
        matches!(created.base_refresh, Some(BaseRefresh::Failed { .. })),
        "{:?}",
        created.base_refresh
    );
}

#[test]
fn a_local_branch_base_runs_no_fetch() {
    let fx = Fixture::new();
    let (bare, project) = clone_project(&fx);
    let clone = fx.path("clone");
    let local = rev(&clone, "HEAD");
    advance_remote(&fx, &bare, "first.txt");

    let mut params = create_params(&project.name, "local");
    params.base = Some("main".to_owned());
    params.refresh_base = true;
    let created = fx
        .workspaces
        .create_workspace(params)
        .expect("create from the local branch");

    assert_eq!(rev(Path::new(&created.path), "HEAD"), local);
    assert_eq!(
        rev(&clone, "refs/remotes/origin/main"),
        local,
        "a local base fetched its remote"
    );
    assert_eq!(created.base_refresh, Some(BaseRefresh::NotRemote));
}

#[test]
fn an_explicit_remote_base_and_the_project_base_are_refreshed() {
    let fx = Fixture::new();
    let (bare, project) = clone_project(&fx);
    advance_remote(&fx, &bare, "first.txt");
    fx.workspaces
        .set_project_base(ProjectSetBaseParams {
            project: project.name.clone(),
            base: Some("origin/main".to_owned()),
        })
        .expect("set the project base");

    let mut by_project = create_params(&project.name, "project-base");
    by_project.refresh_base = true;
    let created = fx
        .workspaces
        .create_workspace(by_project)
        .expect("create from the project base");
    assert_eq!(
        rev(Path::new(&created.path), "HEAD"),
        rev(&bare, "refs/heads/main")
    );

    advance_remote(&fx, &bare, "second.txt");
    let mut explicit = create_params(&project.name, "explicit");
    explicit.base = Some("origin/main".to_owned());
    explicit.refresh_base = true;
    let created = fx
        .workspaces
        .create_workspace(explicit)
        .expect("create from the named base");
    assert_eq!(
        rev(Path::new(&created.path), "HEAD"),
        rev(&bare, "refs/heads/main")
    );
}

#[test]
fn a_refresh_is_refused_for_an_existing_branch_and_for_a_folder_project() {
    let fx = Fixture::new();
    let repo_project = fx.repo_project();
    run_git(&fx.repo(), &["branch", "keep"]);
    let mut existing = create_params(&repo_project.name, "keep");
    existing.existing_branch = true;
    existing.refresh_base = true;
    expect_code(
        fx.workspaces.create_workspace(existing),
        codes::REQUEST_INVALID_PARAMS,
    );

    let folder = fx.folder_project("notes");
    let mut params = create_params(&folder.name, "notes");
    params.refresh_base = true;
    expect_code(
        fx.workspaces.create_workspace(params),
        codes::REQUEST_INVALID_PARAMS,
    );
}
