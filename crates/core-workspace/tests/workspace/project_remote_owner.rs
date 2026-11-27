use std::path::Path;

use crate::fixture::{Fixture, run_git};

fn add_remote(repo: &Path, name: &str, url: &str) {
    run_git(repo, &["remote", "add", name, url]);
}

#[test]
fn origin_owner_is_shown_on_add_show_and_list() {
    let fx = Fixture::new();
    add_remote(&fx.repo(), "origin", "git@github.com:acme/widgets.git");

    let added = fx.repo_project();

    assert_eq!(added.remote_owner.as_deref(), Some("acme"));
    assert_eq!(
        fx.workspaces
            .show_project("repo")
            .expect("show")
            .remote_owner
            .as_deref(),
        Some("acme")
    );
    assert_eq!(
        fx.workspaces.list_projects().expect("list")[0]
            .remote_owner
            .as_deref(),
        Some("acme")
    );
}

#[test]
fn the_only_remote_supplies_the_owner_when_origin_is_missing() {
    let fx = Fixture::new();
    add_remote(
        &fx.repo(),
        "upstream",
        "https://gitlab.example.com/group/sub/repo.git",
    );

    assert_eq!(fx.repo_project().remote_owner.as_deref(), Some("group/sub"));
}

#[test]
fn a_changed_origin_shows_its_new_owner_on_the_next_read() {
    let fx = Fixture::new();
    add_remote(&fx.repo(), "origin", "https://github.com/acme/widgets.git");
    fx.repo_project();

    run_git(
        &fx.repo(),
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/other-org/widgets.git",
        ],
    );

    assert_eq!(
        fx.workspaces
            .show_project("repo")
            .expect("show")
            .remote_owner
            .as_deref(),
        Some("other-org")
    );
}

#[test]
fn a_repository_without_a_remote_has_no_owner() {
    let fx = Fixture::new();

    assert_eq!(fx.repo_project().remote_owner, None);
}

#[test]
fn a_remote_that_does_not_parse_has_no_owner() {
    let fx = Fixture::new();
    add_remote(&fx.repo(), "origin", "/srv/git/repo.git");

    assert_eq!(fx.repo_project().remote_owner, None);
}

#[test]
fn a_folder_project_has_no_owner() {
    let fx = Fixture::new();

    assert_eq!(fx.folder_project("notes").remote_owner, None);
}
