//! `Git::diff_file` for a submodule (gitlink) change: the commits the parent records, not file contents.

use std::path::{Path, PathBuf};

use massorca_core_git::Git;
use massorca_protocol::methods::git::diff::DiffTarget;

use crate::fixture::{Fixture, commit_file, rev, run_git, text};

const SUBMODULE: &str = "vendor/lib";

fn git() -> Git {
    Git::discover().expect("git is installed")
}

/// A library repository with two commits; returns its path and the (first, second) commit ids.
fn library(fixture: &Fixture) -> (PathBuf, String, String) {
    let source = fixture.sibling("lib-source");
    std::fs::create_dir_all(&source).expect("create library dir");
    run_git(&source, &["init", "-q"]);
    commit_file(&source, "lib.txt", "one\n", "lib one");
    let first = rev(&source, "HEAD");
    commit_file(&source, "lib.txt", "two\n", "lib two");
    let second = rev(&source, "HEAD");
    (source, first, second)
}

fn add_submodule(repo: &Path, source: &Path) {
    run_git(
        repo,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            text(source),
            SUBMODULE,
        ],
    );
}

/// A parent repository that records the library's second commit at `vendor/lib`, committed.
/// Returns the parent and the library's (first, second) commits.
fn parent_with_submodule(fixture: &Fixture) -> (PathBuf, String, String) {
    let (source, first, second) = library(fixture);
    let repo = fixture.repo();
    add_submodule(&repo, &source);
    run_git(&repo, &["commit", "-q", "-m", "add submodule"]);
    assert_eq!(rev(&repo.join(SUBMODULE), "HEAD"), second);
    (repo, first, second)
}

/// Moves the submodule's checkout to `commit` without recording it in the parent.
fn check_out_in_submodule(repo: &Path, commit: &str) {
    run_git(&repo.join(SUBMODULE), &["checkout", "-q", commit]);
}

#[test]
fn a_moved_submodule_is_a_pointer_diff_with_both_commits() {
    let fixture = Fixture::new();
    let (repo, first, second) = parent_with_submodule(&fixture);
    check_out_in_submodule(&repo, &first);

    let file = git()
        .diff_file(&repo, &DiffTarget::Worktree, SUBMODULE, None, 1 << 20)
        .expect("diff");
    let pointer = file.submodule.expect("a submodule pointer");
    assert_eq!(pointer.old_commit.as_deref(), Some(second.as_str()));
    assert_eq!(pointer.new_commit.as_deref(), Some(first.as_str()));
    assert_eq!(pointer.dirty, Some(false));
    assert_eq!(
        pointer.summary,
        format!("Subproject commit {second}..{first}")
    );
    assert_eq!((file.patch, file.binary), (None, false));
}

#[test]
fn an_untracked_file_in_a_moved_submodule_marks_it_dirty() {
    let fixture = Fixture::new();
    let (repo, first, _) = parent_with_submodule(&fixture);
    check_out_in_submodule(&repo, &first);
    std::fs::write(repo.join(SUBMODULE).join("scratch.txt"), "x\n").expect("write");

    let pointer = git()
        .diff_file(&repo, &DiffTarget::Head, SUBMODULE, None, 1 << 20)
        .expect("diff")
        .submodule
        .expect("a submodule pointer");
    assert_eq!(pointer.dirty, Some(true));
}

#[test]
fn a_staged_submodule_pointer_compares_the_recorded_commits_without_a_dirty_flag() {
    let fixture = Fixture::new();
    let (repo, first, second) = parent_with_submodule(&fixture);
    check_out_in_submodule(&repo, &first);
    run_git(&repo, &["add", SUBMODULE]);

    let pointer = git()
        .diff_file(&repo, &DiffTarget::Staged, SUBMODULE, None, 1 << 20)
        .expect("diff")
        .submodule
        .expect("a submodule pointer");
    assert_eq!(pointer.old_commit.as_deref(), Some(second.as_str()));
    assert_eq!(pointer.new_commit.as_deref(), Some(first.as_str()));
    assert_eq!(pointer.dirty, None);
}

#[test]
fn a_newly_added_submodule_has_no_old_commit() {
    let fixture = Fixture::new();
    let (source, _, second) = library(&fixture);
    let repo = fixture.repo();
    add_submodule(&repo, &source);

    let pointer = git()
        .diff_file(&repo, &DiffTarget::Staged, SUBMODULE, None, 1 << 20)
        .expect("diff")
        .submodule
        .expect("a submodule pointer");
    assert_eq!(pointer.old_commit, None);
    assert_eq!(pointer.new_commit.as_deref(), Some(second.as_str()));
    assert_eq!(pointer.summary, format!("Subproject commit {second}"));
}

#[test]
fn an_untrusted_caller_reads_the_pointer_and_its_dirty_state() {
    let fixture = Fixture::new();
    let (repo, first, second) = parent_with_submodule(&fixture);
    check_out_in_submodule(&repo, &first);
    std::fs::write(repo.join(SUBMODULE).join("scratch.txt"), "x\n").expect("write");
    let hooks = tempfile::tempdir().expect("hooks dir");

    let pointer = Git::discover()
        .expect("git is installed")
        .for_untrusted_caller(hooks.path())
        .diff_file(&repo, &DiffTarget::Worktree, SUBMODULE, None, 1 << 20)
        .expect("diff")
        .submodule
        .expect("a submodule pointer");
    assert_eq!(pointer.old_commit.as_deref(), Some(second.as_str()));
    assert_eq!(pointer.new_commit.as_deref(), Some(first.as_str()));
    assert_eq!(pointer.dirty, Some(true));
}
