use std::fs;

use massorca_core_git::{Git, GitVersion};
use massorca_protocol::codes;

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
fn a_worktree_whose_directory_is_gone_is_prunable_where_git_reports_it() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "gone");
    fs::remove_dir_all(&ws.path).expect("remove directory");

    let listed = fx.workspaces.list_workspaces(None).expect("list");
    assert!(listed[0].missing);
    // Git prints `prunable` from 2.31; older git leaves the flag absent.
    let since = GitVersion {
        major: 2,
        minor: 31,
        patch: 0,
    };
    let reports_prunable = Git::discover()
        .expect("git on PATH")
        .version()
        .expect("git --version")
        >= since;
    if reports_prunable {
        assert!(listed[0].prunable.is_some());
    } else {
        assert_eq!(listed[0].prunable, None);
    }
}
