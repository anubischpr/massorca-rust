//! `Git::remote_identities` on real repositories: the network remotes, the primary one, and a fork's upstream.

use massorca_core_git::Git;
use massorca_protocol::methods::git::remotes::Provider;

use crate::fixture::{Fixture, run_git};

fn git() -> Git {
    Git::discover().expect("git is installed")
}

#[test]
fn every_network_remote_is_named_and_a_local_one_is_left_out() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(
        &repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/widgets.git",
        ],
    );
    run_git(&repo, &["remote", "add", "backup", "/srv/widgets.git"]);
    let remotes = git().remote_identities(&repo).expect("remotes");
    let names: Vec<&str> = remotes
        .iter()
        .map(|remote| remote.remote.as_str())
        .collect();
    assert_eq!(names, ["origin"]);
    assert_eq!(remotes[0].host, "github.com");
    assert_eq!(remotes[0].provider, Provider::Github);
    assert_eq!(remotes[0].owner, "acme");
    assert_eq!(remotes[0].repo, "widgets");
    assert!(remotes[0].primary);
    assert_eq!(remotes[0].fork_of, None);
}

#[test]
fn origin_is_a_fork_when_upstream_names_the_same_repository_under_another_owner() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(
        &repo,
        &["remote", "add", "origin", "git@github.com:acme/widgets.git"],
    );
    run_git(
        &repo,
        &[
            "remote",
            "add",
            "upstream",
            "https://github.com/ops/widgets",
        ],
    );
    let remotes = git().remote_identities(&repo).expect("remotes");
    assert_eq!(remotes.len(), 2);
    assert_eq!(remotes[0].remote, "origin");
    assert_eq!(remotes[0].fork_of.as_deref(), Some("ops/widgets"));
    assert!(remotes[0].primary);
    assert_eq!(remotes[1].remote, "upstream");
    assert_eq!(remotes[1].fork_of, None);
    assert!(!remotes[1].primary);
}

#[test]
fn a_repository_without_network_remotes_has_none() {
    let fixture = Fixture::new();
    let remotes = git().remote_identities(&fixture.repo()).expect("remotes");
    assert!(remotes.is_empty());
}

#[test]
fn the_first_network_remote_is_primary_without_origin() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(
        &repo,
        &[
            "remote",
            "add",
            "fork",
            "https://gitlab.com/group/sub/widgets.git",
        ],
    );
    let remotes = git().remote_identities(&repo).expect("remotes");
    assert_eq!(remotes[0].provider, Provider::Gitlab);
    assert_eq!(remotes[0].owner, "group/sub");
    assert!(remotes[0].primary);
}
