use massorca_core_git::{Git, GitError, WorktreeChanges};

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

#[test]
fn ignored_files_are_counted_apart_from_untracked_and_uncommitted_changes() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    commit_file_in(
        &repo,
        ".gitignore",
        "*.env
",
    );
    std::fs::write(repo.join("secrets.env"), "key").expect("write ignored file");
    std::fs::write(
        repo.join("notes.txt"),
        "draft
",
    )
    .expect("write untracked file");

    let changes = git().status_changes(&repo).expect("status");

    assert_eq!(
        changes,
        WorktreeChanges {
            uncommitted: 0,
            untracked: 1,
            ignored: 1,
        }
    );
    assert!(!changes.is_clean());
}

#[test]
fn a_tree_with_only_ignored_files_is_not_clean_but_has_no_other_changes() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    commit_file_in(
        &repo,
        ".gitignore",
        "build/
",
    );
    std::fs::create_dir_all(repo.join("build")).expect("create ignored dir");
    std::fs::write(repo.join("build").join("out.bin"), "x").expect("write ignored file");

    let changes = git().status_changes(&repo).expect("status");

    assert_eq!(changes.uncommitted, 0);
    assert_eq!(changes.untracked, 0);
    assert_eq!(changes.ignored, 1, "one ignored entry for the directory");
    assert!(!changes.is_clean());
}

fn commit_file_in(repo: &std::path::Path, name: &str, contents: &str) {
    crate::fixture::commit_file(repo, name, contents, "add ignore rules");
}

/// `repository_dirs` reads `.git` without spawning git; it must agree with `rev-parse` for the main and a linked work tree.
#[test]
fn repository_dirs_agree_with_rev_parse() {
    let fixture = Fixture::new();
    let linked = fixture.sibling("linked");
    run_git(
        &fixture.repo(),
        &["worktree", "add", "-q", "-b", "linked", text(&linked)],
    );
    for dir in [fixture.repo(), linked] {
        let (git_dir, common) = git().repository_dirs(&dir).expect("repository dirs");
        let expected_git = run_git(&dir, &["rev-parse", "--absolute-git-dir"]);
        let expected_common = run_git(&dir, &["rev-parse", "--git-common-dir"]);
        let expected_common = dir.join(expected_common.trim());
        assert_eq!(
            canonical(&git_dir),
            canonical(std::path::Path::new(expected_git.trim()))
        );
        assert_eq!(canonical(&common), canonical(&expected_common));
    }
}

#[test]
fn main_worktree_of_the_main_repository_is_the_repository() {
    let fixture = Fixture::new();
    let main = git().main_worktree(&fixture.repo()).expect("main worktree");
    assert_eq!(
        main.map(|path| canonical(&path)),
        Some(canonical(&fixture.repo()))
    );
}

#[test]
fn main_worktree_from_a_linked_worktree_is_the_main_repository() {
    let fixture = Fixture::new();
    let linked = fixture.sibling("linked");
    run_git(
        &fixture.repo(),
        &["worktree", "add", "-q", "-b", "linked", text(&linked)],
    );
    let main = git().main_worktree(&linked).expect("main worktree");
    assert_eq!(
        main.map(|path| canonical(&path)),
        Some(canonical(&fixture.repo()))
    );
}

#[test]
fn main_worktree_from_a_subfolder_of_a_linked_worktree_is_the_main_repository() {
    let fixture = Fixture::new();
    let linked = fixture.sibling("linked");
    run_git(
        &fixture.repo(),
        &["worktree", "add", "-q", "-b", "linked", text(&linked)],
    );
    let nested = linked.join("src").join("deep");
    std::fs::create_dir_all(&nested).expect("create nested dir");
    let main = git().main_worktree(&nested).expect("main worktree");
    assert_eq!(
        main.map(|path| canonical(&path)),
        Some(canonical(&fixture.repo()))
    );
}

#[test]
fn main_worktree_outside_every_repository_is_none() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert_eq!(
        git().main_worktree(dir.path()).expect("main worktree"),
        None
    );
}

#[test]
fn main_worktree_of_a_linked_worktree_of_a_bare_repository_is_none() {
    let fixture = Fixture::new();
    let bare = fixture.sibling("bare.git");
    run_git(
        &fixture.repo(),
        &["clone", "-q", "--bare", text(&fixture.repo()), text(&bare)],
    );
    let linked = fixture.sibling("linked");
    run_git(&bare, &["worktree", "add", "-q", text(&linked), "main"]);
    assert!(git().toplevel(&linked).expect("toplevel").is_some());
    assert_eq!(git().main_worktree(&linked).expect("main worktree"), None);
}

#[test]
fn a_linked_worktree_shares_the_main_repositorys_common_dir() {
    let fixture = Fixture::new();
    let linked = fixture.sibling("linked");
    run_git(
        &fixture.repo(),
        &["worktree", "add", "-q", "-b", "linked", text(&linked)],
    );
    let main = git().common_dir(&fixture.repo()).expect("main common dir");
    let from_linked = git().common_dir(&linked).expect("linked common dir");
    assert_eq!(canonical(&main), canonical(&fixture.repo().join(".git")));
    assert_eq!(canonical(&from_linked), canonical(&main));
}
