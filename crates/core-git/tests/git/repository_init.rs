//! `Git::init_repository`: `git init` and the empty first commit a worktree branches from (decision 0115).

use std::path::PathBuf;

use massorca_core_git::{Git, discard_new_repository};
use tempfile::TempDir;

use crate::fixture::{run_git, text};

/// A folder holding `notes.txt`, inside a temporary directory that is not a repository.
fn notes_folder() -> (TempDir, PathBuf) {
    let root = tempfile::tempdir().expect("temp dir");
    let dir = root.path().join("notes");
    std::fs::create_dir_all(&dir).expect("create folder");
    std::fs::write(dir.join("notes.txt"), "keep me").expect("write notes");
    (root, dir)
}

fn git() -> Git {
    Git::discover().expect("git on PATH")
}

#[test]
fn a_new_repository_has_one_empty_commit_and_keeps_the_folder_files() {
    let (_root, dir) = notes_folder();

    git().init_repository(&dir).expect("init the folder");

    assert_eq!(run_git(&dir, &["rev-list", "--count", "HEAD"]).trim(), "1");
    assert_eq!(
        run_git(&dir, &["log", "-1", "--format=%s"]).trim(),
        "Initial commit"
    );
    assert!(run_git(&dir, &["ls-tree", "-r", "HEAD"]).trim().is_empty());
    assert!(dir.join("notes.txt").is_file());
    assert!(run_git(&dir, &["status", "--porcelain"]).contains("?? notes.txt"));
}

#[test]
fn a_worktree_can_branch_from_the_first_commit() {
    let (root, dir) = notes_folder();
    git().init_repository(&dir).expect("init the folder");
    let linked = root.path().join("feature");

    run_git(
        &dir,
        &["worktree", "add", "-q", "-b", "feature", text(&linked)],
    );

    assert_eq!(
        run_git(&linked, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
        "feature"
    );
}

#[test]
fn a_missing_folder_is_refused_and_nothing_is_created() {
    let root = tempfile::tempdir().expect("temp dir");
    let missing = root.path().join("gone");

    assert!(git().init_repository(&missing).is_err());
    assert!(!missing.exists());
}

#[test]
fn discarding_a_new_repository_leaves_the_folder_files() {
    let (_root, dir) = notes_folder();
    git().init_repository(&dir).expect("init the folder");
    assert!(dir.join(".git").is_dir(), "init made the repository");

    discard_new_repository(&dir);

    assert!(!dir.join(".git").exists());
    assert!(dir.join("notes.txt").is_file());
}
