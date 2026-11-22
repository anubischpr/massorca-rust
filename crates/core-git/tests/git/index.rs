//! Staging, unstaging and discarding against real repositories, with every refusal of decision 0013.

use massorca_core_git::{DiscardError, DiscardWaivers, Git, PathSet, failure_code};
use massorca_protocol::codes;
use massorca_protocol::methods::git::ChangeKind;

use crate::fixture::{Fixture, commit_file, run_git};

fn git() -> Git {
    Git::discover().expect("git is installed")
}

fn paths(list: &[&str]) -> Vec<String> {
    list.iter().map(|path| (*path).to_owned()).collect()
}

fn refusal(error: DiscardError) -> (&'static str, Vec<String>) {
    match error {
        DiscardError::Refused { code, paths, .. } => (code, paths),
        DiscardError::Git(error) => panic!("expected a refusal, got {error}"),
    }
}

fn read(repo: &std::path::Path, name: &str) -> String {
    std::fs::read_to_string(repo.join(name)).expect("read")
}

#[test]
fn stage_and_unstage_named_files_including_deletions_and_glob_names() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    commit_file(&repo, "gone.txt", "x\n", "add gone");
    std::fs::remove_file(repo.join("gone.txt")).expect("delete");
    std::fs::write(repo.join("[ab].txt"), "glob\n").expect("write");
    std::fs::write(repo.join("a.txt"), "not named\n").expect("write");

    let named = paths(&["gone.txt", "[ab].txt"]);
    git().stage(&repo, PathSet::Paths(&named)).expect("stage");
    let status = git().status(&repo, 100).expect("status");
    let staged: Vec<_> = status
        .entries
        .iter()
        .filter(|e| e.staged.is_some())
        .map(|e| e.path.as_str())
        .collect();
    assert_eq!(staged, ["[ab].txt", "gone.txt"]);

    git()
        .unstage(&repo, PathSet::Paths(&paths(&["[ab].txt"])))
        .expect("unstage");
    let status = git().status(&repo, 100).expect("status");
    assert!(
        status
            .entries
            .iter()
            .any(|e| e.path == "[ab].txt" && e.untracked)
    );
    assert!(
        status
            .entries
            .iter()
            .any(|e| e.path == "gone.txt" && e.staged == Some(ChangeKind::Deleted))
    );
}

#[test]
fn stage_all_then_unstage_all_works_before_the_first_commit() {
    let dir = tempfile::tempdir().expect("temp dir");
    run_git(dir.path(), &["init", "-q"]);
    std::fs::write(dir.path().join("one.txt"), "1\n").expect("write");
    git().stage(dir.path(), PathSet::All).expect("stage all");
    assert_eq!(
        git().status(dir.path(), 100).expect("status").totals.staged,
        1
    );
    git()
        .unstage(dir.path(), PathSet::All)
        .expect("unstage all");
    let status = git().status(dir.path(), 100).expect("status");
    assert_eq!((status.totals.staged, status.totals.untracked), (0, 1));
}

#[test]
fn many_paths_are_staged_in_chunks() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let names: Vec<String> = (0..1500)
        .map(|n| format!("{n:04}-{}.txt", "x".repeat(20)))
        .collect();
    for name in &names {
        std::fs::write(repo.join(name), "x\n").expect("write");
    }
    git().stage(&repo, PathSet::Paths(&names)).expect("stage");
    assert_eq!(git().status(&repo, 10).expect("status").totals.staged, 1500);
}

#[test]
fn discard_restores_unstaged_edits_and_keeps_staged_ones() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    std::fs::write(repo.join("README.md"), "staged\n").expect("write");
    run_git(&repo, &["add", "README.md"]);
    std::fs::write(repo.join("README.md"), "staged\nunstaged\n").expect("write");

    let done = git()
        .discard(
            &repo,
            PathSet::Paths(&paths(&["README.md"])),
            DiscardWaivers::default(),
            false,
        )
        .expect("discard");
    assert_eq!(done.restored_from_index, ["README.md"]);
    assert_eq!(read(&repo, "README.md"), "staged\n");
    let status = git().status(&repo, 100).expect("status");
    assert_eq!(status.entries[0].staged, Some(ChangeKind::Modified));
    assert_eq!(status.entries[0].unstaged, None);
}

#[test]
fn untracked_files_are_refused_without_the_waiver_and_deleted_with_it() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    std::fs::write(repo.join("new.txt"), "work\n").expect("write");
    std::fs::write(repo.join("README.md"), "edited\n").expect("write");
    let both = paths(&["README.md", "new.txt"]);

    let error = git()
        .discard(
            &repo,
            PathSet::Paths(&both),
            DiscardWaivers::default(),
            false,
        )
        .expect_err("refused");
    assert_eq!(
        refusal(error),
        (codes::GIT_DISCARD_UNTRACKED, paths(&["new.txt"]))
    );
    // A refusal changes nothing, not even the files the waivers would have covered.
    assert_eq!(read(&repo, "README.md"), "edited\n");
    assert!(repo.join("new.txt").exists());

    let waivers = DiscardWaivers {
        delete_untracked: true,
        include_staged: false,
    };
    let planned = git()
        .discard(&repo, PathSet::Paths(&both), waivers, true)
        .expect("dry run");
    assert!(planned.dry_run);
    assert_eq!(planned.deleted, ["new.txt"]);
    assert!(repo.join("new.txt").exists(), "a dry run deleted a file");

    git()
        .discard(&repo, PathSet::Paths(&both), waivers, false)
        .expect("discard");
    assert!(!repo.join("new.txt").exists());
    assert_eq!(read(&repo, "README.md"), "hello\n");
}

#[test]
fn staged_only_changes_need_include_staged_and_a_staged_addition_also_needs_delete_untracked() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    std::fs::write(repo.join("README.md"), "staged\n").expect("write");
    std::fs::write(repo.join("added.txt"), "new\n").expect("write");
    run_git(&repo, &["add", "README.md", "added.txt"]);

    let error = git()
        .discard(
            &repo,
            PathSet::Paths(&paths(&["README.md"])),
            DiscardWaivers::default(),
            false,
        )
        .expect_err("refused");
    assert_eq!(refusal(error).0, codes::GIT_DISCARD_STAGED);

    let staged = DiscardWaivers {
        delete_untracked: false,
        include_staged: true,
    };
    let error = git()
        .discard(&repo, PathSet::All, staged, false)
        .expect_err("refused");
    assert_eq!(
        refusal(error),
        (codes::GIT_DISCARD_UNTRACKED, paths(&["added.txt"]))
    );

    let everything = DiscardWaivers {
        delete_untracked: true,
        include_staged: true,
    };
    let done = git()
        .discard(&repo, PathSet::All, everything, false)
        .expect("discard");
    assert_eq!(done.restored_from_head, ["README.md"]);
    assert_eq!(done.deleted, ["added.txt"]);
    assert_eq!(read(&repo, "README.md"), "hello\n");
    assert!(!repo.join("added.txt").exists());
    assert!(git().status(&repo, 100).expect("status").entries.is_empty());
}

#[test]
fn directories_and_conflicts_are_refused_and_ignored_files_are_never_touched() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    commit_file(&repo, ".gitignore", "*.log\n", "ignore logs");
    std::fs::create_dir_all(repo.join("dir")).expect("mkdir");
    std::fs::write(repo.join("dir").join("a.txt"), "a\n").expect("write");
    std::fs::write(repo.join("keep.log"), "ignored\n").expect("write");

    let error = git()
        .discard(
            &repo,
            PathSet::Paths(&paths(&["dir"])),
            DiscardWaivers::default(),
            false,
        )
        .expect_err("refused");
    assert_eq!(
        refusal(error),
        (codes::GIT_PATH_IS_DIRECTORY, paths(&["dir"]))
    );

    let everything = DiscardWaivers {
        delete_untracked: true,
        include_staged: true,
    };
    git()
        .discard(&repo, PathSet::All, everything, false)
        .expect("discard");
    assert!(
        repo.join("keep.log").exists(),
        "an ignored file was deleted"
    );
    assert!(!repo.join("dir").join("a.txt").exists());

    run_git(&repo, &["checkout", "-q", "-b", "other"]);
    commit_file(&repo, "README.md", "theirs\n", "theirs");
    run_git(&repo, &["checkout", "-q", "main"]);
    commit_file(&repo, "README.md", "ours\n", "ours");
    let merge = massorca_process::command("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "merge",
            "other",
        ])
        .current_dir(&repo)
        .output()
        .expect("merge");
    assert!(!merge.status.success());
    let error = git()
        .discard(&repo, PathSet::All, everything, false)
        .expect_err("refused");
    assert_eq!(
        refusal(error),
        (codes::GIT_PATH_CONFLICTED, paths(&["README.md"]))
    );
}

#[test]
fn a_held_index_lock_is_reported_as_such() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    std::fs::write(repo.join("README.md"), "edited").expect("write");
    std::fs::write(repo.join(".git").join("index.lock"), "").expect("take the lock");
    let error = git()
        .stage(&repo, PathSet::All)
        .expect_err("the index is locked");
    assert_eq!(
        failure_code(&error),
        Some(codes::GIT_INDEX_LOCKED),
        "{error}"
    );
}
