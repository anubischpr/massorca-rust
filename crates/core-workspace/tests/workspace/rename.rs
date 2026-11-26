//! `workspace.rename`: a name the service chose is replaced, with its branch; the folder stays (decision 0111).

use std::path::Path;

use massorca_core_store::WorkspaceState;
use massorca_core_workspace::BranchPrefix;
use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceRenameParams};

use crate::fixture::{Fixture, branch_exists, create_params, expect_code, run_git, text};

fn rename(workspace: &str, name: &str) -> WorkspaceRenameParams {
    WorkspaceRenameParams {
        workspace: workspace.to_owned(),
        name: name.to_owned(),
        from_generated: true,
        expected_branch: None,
    }
}

/// A workspace under a generated creature name.
fn generated(fx: &Fixture, project: &str) -> Workspace {
    fx.workspaces
        .create_workspace_once(create_params(project, ""))
        .expect("create")
        .workspace
}

#[test]
fn a_generated_worktree_gets_the_new_name_and_branch_and_keeps_its_folder() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = generated(&fx, &project.name);
    let old_branch = ws.branch.clone().expect("a worktree has a branch");

    let renamed = fx
        .workspaces
        .rename_workspace(rename(&ws.id, "Fix login"))
        .expect("rename");

    assert_eq!(renamed.id, ws.id);
    assert_eq!(renamed.name, "Fix-login");
    assert_eq!(renamed.branch.as_deref(), Some("Fix-login"));
    assert_eq!(renamed.path, ws.path);
    assert!(renamed.name_generated);
    assert!(Path::new(&ws.path).is_dir());
    assert!(branch_exists(&fx.repo(), "Fix-login"));
    assert!(!branch_exists(&fx.repo(), &old_branch));
    assert_eq!(
        run_git(Path::new(&ws.path), &["symbolic-ref", "--short", "HEAD"]).trim(),
        "Fix-login"
    );
    assert_eq!(fx.workspaces.show_workspace(&ws.id).unwrap(), renamed);
}

#[test]
fn the_new_branch_takes_the_branch_prefix() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let prefixed = fx
        .workspaces
        .with_branch_prefix(BranchPrefix::Custom("team".to_owned()));
    let ws = prefixed
        .create_workspace_once(create_params(&project.name, ""))
        .expect("create")
        .workspace;

    let renamed = prefixed
        .rename_workspace(rename(&ws.id, "fix-login"))
        .expect("rename");

    assert_eq!(renamed.branch.as_deref(), Some("team/fix-login"));
}

#[test]
fn a_typed_name_is_never_renamed() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "feature");

    expect_code(
        fx.workspaces.rename_workspace(rename(&ws.id, "other")),
        codes::WORKSPACE_NAME_NOT_GENERATED,
    );
    assert!(branch_exists(&fx.repo(), "feature"));
}

#[test]
fn a_call_without_from_generated_is_refused() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = generated(&fx, &project.name);

    expect_code(
        fx.workspaces.rename_workspace(WorkspaceRenameParams {
            from_generated: false,
            ..rename(&ws.id, "other")
        }),
        codes::REQUEST_INVALID_PARAMS,
    );
}

#[test]
fn a_branch_other_than_the_expected_one_is_refused() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = generated(&fx, &project.name);

    expect_code(
        fx.workspaces.rename_workspace(WorkspaceRenameParams {
            expected_branch: Some("something-else".to_owned()),
            ..rename(&ws.id, "fix-login")
        }),
        codes::WORKSPACE_BRANCH_CHANGED,
    );
    assert_eq!(fx.workspaces.show_workspace(&ws.id).unwrap().name, ws.name);

    let renamed = fx
        .workspaces
        .rename_workspace(WorkspaceRenameParams {
            expected_branch: ws.branch.clone(),
            ..rename(&ws.id, "fix-login")
        })
        .expect("the expected branch matches");
    assert_eq!(renamed.name, "fix-login");
}

#[test]
fn a_branch_switched_in_the_worktree_is_refused() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = generated(&fx, &project.name);
    run_git(Path::new(&ws.path), &["checkout", "-q", "-b", "elsewhere"]);

    expect_code(
        fx.workspaces.rename_workspace(rename(&ws.id, "fix-login")),
        codes::WORKSPACE_BRANCH_CHANGED,
    );
}

#[test]
fn a_detached_head_is_refused() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = generated(&fx, &project.name);
    run_git(Path::new(&ws.path), &["checkout", "-q", "--detach"]);

    expect_code(
        fx.workspaces.rename_workspace(rename(&ws.id, "fix-login")),
        codes::GIT_DETACHED_HEAD,
    );
}

#[test]
fn a_pushed_branch_is_refused() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let bare = text(&fx.path("remote.git"));
    run_git(&fx.repo(), &["init", "-q", "--bare", &bare]);
    run_git(&fx.repo(), &["remote", "add", "origin", &bare]);
    let ws = generated(&fx, &project.name);
    let branch = ws.branch.clone().expect("a branch");
    run_git(
        Path::new(&ws.path),
        &["push", "-q", "-u", "origin", &branch],
    );

    expect_code(
        fx.workspaces.rename_workspace(rename(&ws.id, "fix-login")),
        codes::WORKSPACE_BRANCH_PUBLISHED,
    );
    assert!(branch_exists(&fx.repo(), &branch));
}

#[test]
fn a_branch_on_a_remote_without_an_upstream_is_refused() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    run_git(
        &fx.repo(),
        &[
            "remote",
            "add",
            "origin",
            "https://example.invalid/repo.git",
        ],
    );
    let ws = generated(&fx, &project.name);
    let tracking = format!(
        "refs/remotes/origin/{}",
        ws.branch.clone().expect("a branch")
    );
    run_git(&fx.repo(), &["update-ref", &tracking, "HEAD"]);

    expect_code(
        fx.workspaces.rename_workspace(rename(&ws.id, "fix-login")),
        codes::WORKSPACE_BRANCH_PUBLISHED,
    );
}

#[test]
fn a_taken_name_moves_on_to_the_next_suffix() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    fx.create(&project.name, "fix-login");
    let ws = generated(&fx, &project.name);

    let renamed = fx
        .workspaces
        .rename_workspace(rename(&ws.id, "fix-login"))
        .expect("rename");

    assert_eq!(renamed.name, "fix-login-2");
    assert_eq!(renamed.branch.as_deref(), Some("fix-login-2"));
}

#[test]
fn a_folder_that_holds_the_name_does_not_move_it_on() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = generated(&fx, &project.name);
    let folder = fx.path("defaults/repo/fix-login");
    std::fs::create_dir_all(&folder).expect("make folder");
    std::fs::write(folder.join("x.txt"), "x").expect("fill folder");

    let renamed = fx
        .workspaces
        .rename_workspace(rename(&ws.id, "fix-login"))
        .expect("rename");

    assert_eq!(renamed.name, "fix-login");
}

#[test]
fn a_failed_row_write_renames_the_branch_back() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = generated(&fx, &project.name);
    let branch = ws.branch.clone().expect("a branch");
    fx.fail_workspace_updates();

    expect_code(
        fx.workspaces.rename_workspace(rename(&ws.id, "fix-login")),
        codes::INTERNAL,
    );
    assert!(branch_exists(&fx.repo(), &branch));
    assert!(!branch_exists(&fx.repo(), "fix-login"));
    assert_eq!(fx.workspaces.show_workspace(&ws.id).unwrap().name, ws.name);
}

#[test]
fn a_folder_workspace_changes_its_name_only() {
    let fx = Fixture::new();
    let folder = fx.folder_project("notes");
    let ws = generated(&fx, &folder.name);

    let renamed = fx
        .workspaces
        .rename_workspace(rename(&ws.id, "meeting-notes"))
        .expect("rename");

    assert_eq!(renamed.name, "meeting-notes");
    assert_eq!(renamed.branch, None);
    assert_eq!(renamed.path, ws.path);
    assert!(renamed.name_generated);
}

#[test]
fn renaming_to_its_own_name_changes_nothing_and_gets_no_suffix() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = generated(&fx, &project.name);

    let same = fx
        .workspaces
        .rename_workspace(rename(&ws.id, &ws.name))
        .expect("a rename to its own name");

    assert_eq!(same.name, ws.name);
    assert_eq!(same.branch, ws.branch);
}

#[test]
fn a_stale_expected_branch_is_refused_even_when_the_name_is_unchanged() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = generated(&fx, &project.name);

    expect_code(
        fx.workspaces.rename_workspace(WorkspaceRenameParams {
            expected_branch: Some("something-else".to_owned()),
            ..rename(&ws.id, &ws.name)
        }),
        codes::WORKSPACE_BRANCH_CHANGED,
    );
}

#[test]
fn a_case_only_change_renames_the_name_and_branch_without_a_suffix() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = generated(&fx, &project.name);
    let first = fx
        .workspaces
        .rename_workspace(rename(&ws.id, "otter"))
        .expect("first rename");
    assert_eq!(first.name, "otter");

    let renamed = fx
        .workspaces
        .rename_workspace(rename(&ws.id, "Otter"))
        .expect("case-only rename");

    assert_eq!(renamed.name, "Otter");
    assert_eq!(renamed.branch.as_deref(), Some("Otter"));
    assert_eq!(
        run_git(Path::new(&ws.path), &["symbolic-ref", "--short", "HEAD"]).trim(),
        "Otter"
    );
}

#[test]
fn a_workspace_being_removed_is_refused() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = generated(&fx, &project.name);
    fx.store
        .set_workspace_state(&ws.id, WorkspaceState::Removing)
        .expect("mark removing");

    expect_code(
        fx.workspaces.rename_workspace(rename(&ws.id, "fix-login")),
        codes::WORKSPACE_BUSY,
    );
}

#[test]
fn another_workspace_of_another_case_moves_the_name_on() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    fx.create(&project.name, "Fix-Login");
    let ws = generated(&fx, &project.name);

    let renamed = fx
        .workspaces
        .rename_workspace(rename(&ws.id, "fix-login"))
        .expect("rename");

    assert_eq!(renamed.name, "fix-login-2");
}

#[test]
fn a_branch_without_a_workspace_moves_the_name_on() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    run_git(&fx.repo(), &["branch", "fix-login"]);
    let ws = generated(&fx, &project.name);

    let renamed = fx
        .workspaces
        .rename_workspace(rename(&ws.id, "fix-login"))
        .expect("rename");

    assert_eq!(renamed.name, "fix-login-2");
    assert_eq!(renamed.branch.as_deref(), Some("fix-login-2"));
}

#[test]
fn a_rename_by_the_first_prompt_trigger_ends_the_generated_state() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = generated(&fx, &project.name);

    let renamed = fx
        .workspaces
        .rename_from_first_prompt(rename(&ws.id, "Fix login"))
        .expect("rename");

    assert_eq!(renamed.name, "Fix-login");
    assert!(!renamed.name_generated, "{renamed:?}");
    assert!(!fx.workspaces.show_workspace(&ws.id).unwrap().name_generated);
    expect_code(
        fx.workspaces.rename_workspace(rename(&ws.id, "other")),
        codes::WORKSPACE_NAME_NOT_GENERATED,
    );
}
