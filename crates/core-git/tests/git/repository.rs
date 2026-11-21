use massorca_core_git::{Git, GitError};

use crate::fixture::{Fixture, canonical, run_git, text};

fn git() -> Git {
    Git::discover().expect("git on PATH")
}

#[test]
fn toplevel_of_the_repository_is_the_repository() {
    let fixture = Fixture::new();
    let top = git().toplevel(&fixture.repo()).expect("toplevel");
    assert_eq!(
        top.map(|path| canonical(&path)),
        Some(canonical(&fixture.repo()))
    );
}

#[test]
fn toplevel_of_a_subdirectory_is_the_work_tree_root() {
    let fixture = Fixture::new();
    let nested = fixture.repo().join("src").join("deep");
    std::fs::create_dir_all(&nested).expect("create nested dir");
    let top = git().toplevel(&nested).expect("toplevel");
    assert_eq!(
        top.map(|path| canonical(&path)),
        Some(canonical(&fixture.repo()))
    );
}

#[test]
fn toplevel_inside_a_linked_worktree_is_that_worktree() {
    let fixture = Fixture::new();
    let linked = fixture.sibling("linked");
    run_git(
        &fixture.repo(),
        &["worktree", "add", "-q", "-b", "linked", text(&linked)],
    );
    let nested = linked.join("sub");
    std::fs::create_dir_all(&nested).expect("create nested dir");
    let top = git().toplevel(&nested).expect("toplevel");
    assert_eq!(top.map(|path| canonical(&path)), Some(canonical(&linked)));
}

#[test]
fn toplevel_outside_any_repository_is_none() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert_eq!(git().toplevel(dir.path()).expect("toplevel"), None);
}

#[test]
fn toplevel_of_a_missing_directory_is_an_error_not_none() {
    let fixture = Fixture::new();
    let error = git()
        .toplevel(&fixture.sibling("missing"))
        .expect_err("missing directory");
    assert!(matches!(error, GitError::Spawn(_)), "{error:?}");
}

#[test]
fn a_fresh_commit_leaves_the_status_clean() {
    let fixture = Fixture::new();
    assert!(!git().status_dirty(&fixture.repo()).expect("status"));
}

#[test]
fn an_untracked_file_makes_the_status_dirty() {
    let fixture = Fixture::new();
    std::fs::write(fixture.repo().join("notes.txt"), "draft\n").expect("write file");
    assert!(git().status_dirty(&fixture.repo()).expect("status"));
}

#[test]
fn an_edited_tracked_file_makes_the_status_dirty() {
    let fixture = Fixture::new();
    std::fs::write(fixture.repo().join("README.md"), "changed\n").expect("write file");
    assert!(git().status_dirty(&fixture.repo()).expect("status"));
}

#[test]
fn status_outside_a_repository_is_an_error_not_clean() {
    let dir = tempfile::tempdir().expect("temp dir");
    let error = git()
        .status_dirty(dir.path())
        .expect_err("not a repository");
    assert!(matches!(error, GitError::Failed { .. }), "{error:?}");
}
