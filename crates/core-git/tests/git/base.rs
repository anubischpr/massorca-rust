//! `Git::default_base` and `Git::qualify_base` against real repositories.

use massorca_core_git::{Base, Git};

use crate::fixture::{Fixture, commit_file, rev, run_git, text};

fn git() -> Git {
    Git::discover().expect("git is installed")
}

fn base(name: &str, revision: &str) -> Base {
    Base {
        name: name.to_owned(),
        revision: revision.to_owned(),
    }
}

#[test]
fn without_a_remote_the_default_base_is_the_local_main_branch() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["checkout", "-q", "-b", "topic"]);
    assert_eq!(
        git().default_base(&repo).expect("default"),
        Some(base("main", "refs/heads/main"))
    );
}

#[test]
fn a_clone_takes_the_branch_origin_head_points_at() {
    let fixture = Fixture::new();
    let clone = fixture.sibling("clone");
    run_git(
        &fixture.repo(),
        &["clone", "-q", text(&fixture.repo()), text(&clone)],
    );
    run_git(&clone, &["checkout", "-q", "-b", "topic"]);
    assert_eq!(
        git().default_base(&clone).expect("default"),
        Some(base("origin/main", "refs/remotes/origin/main"))
    );
}

#[test]
fn a_repository_without_any_default_branch_has_no_default_base() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["branch", "-q", "-m", "main", "trunk"]);
    assert_eq!(git().default_base(&repo).expect("default"), None);
}

#[test]
fn a_named_base_prefers_the_branch_over_a_tag_and_the_remote_for_a_slash() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["branch", "release"]);
    commit_file(&repo, "later.txt", "later\n", "later");
    run_git(&repo, &["tag", "release"]);
    let qualified = git().qualify_base(&repo, "release").expect("qualify");
    assert_eq!(qualified, base("release", "refs/heads/release"));
    assert_ne!(
        rev(&repo, "refs/heads/release"),
        rev(&repo, "refs/tags/release")
    );

    let clone = fixture.sibling("clone");
    run_git(&repo, &["clone", "-q", text(&repo), text(&clone)]);
    assert_eq!(
        git().qualify_base(&clone, "origin/main").expect("qualify"),
        base("origin/main", "refs/remotes/origin/main")
    );
    let head = rev(&clone, "HEAD");
    assert_eq!(
        git().qualify_base(&clone, &head).expect("qualify"),
        base(&head, &head),
        "a commit id passes through"
    );
}
