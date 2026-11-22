//! `Git::status` against real repositories: every entry kind, line counts, branch state and operations.

use massorca_core_git::Git;
use massorca_protocol::methods::git::ChangeKind;
use massorca_protocol::methods::git::status::{ConflictKind, GitOperation, StatusEntry};

use crate::fixture::{Fixture, commit_file, rev, run_git, text};

fn git() -> Git {
    Git::discover().expect("git is installed")
}

fn entry<'a>(entries: &'a [StatusEntry], path: &str) -> &'a StatusEntry {
    entries
        .iter()
        .find(|entry| entry.path == path)
        .unwrap_or_else(|| panic!("no entry for {path}: {entries:?}"))
}

#[test]
fn a_clean_repository_has_no_entries_and_names_its_branch() {
    let fixture = Fixture::new();
    let status = git().status(&fixture.repo(), 100).expect("status");
    assert!(status.entries.is_empty(), "{status:?}");
    assert_eq!(status.branch.head.as_deref(), Some("main"));
    assert_eq!(status.branch.oid, Some(rev(&fixture.repo(), "HEAD")));
    assert_eq!(status.operation, None);
}

#[test]
fn staged_unstaged_and_untracked_changes_carry_their_line_counts() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    commit_file(&repo, "both.txt", "a\nb\n", "add both");
    std::fs::write(repo.join("README.md"), "hello\nworld\n").expect("write");
    std::fs::write(repo.join("both.txt"), "a\nB\n").expect("write");
    run_git(&repo, &["add", "both.txt"]);
    std::fs::write(repo.join("both.txt"), "a\nB\nc\n").expect("write");
    std::fs::create_dir_all(repo.join("dir name")).expect("mkdir");
    std::fs::write(repo.join("dir name").join("new file.txt"), "x\n").expect("write");

    let status = git().status(&repo, 100).expect("status");
    let readme = entry(&status.entries, "README.md");
    assert_eq!(readme.unstaged, Some(ChangeKind::Modified));
    assert_eq!(readme.staged, None);
    assert_eq!(
        readme.unstaged_lines.map(|l| (l.added, l.removed)),
        Some((1, 0))
    );
    let both = entry(&status.entries, "both.txt");
    assert_eq!(both.staged, Some(ChangeKind::Modified));
    assert_eq!(
        both.staged_lines.map(|l| (l.added, l.removed)),
        Some((1, 1))
    );
    assert_eq!(
        both.unstaged_lines.map(|l| (l.added, l.removed)),
        Some((1, 0))
    );
    // Untracked files are listed one by one, not as their directory.
    assert!(entry(&status.entries, "dir name/new file.txt").untracked);
    assert_eq!(
        (
            status.totals.staged,
            status.totals.unstaged,
            status.totals.untracked
        ),
        (1, 2, 1)
    );
}

#[test]
fn a_binary_change_is_marked_and_counts_no_lines() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    std::fs::write(repo.join("blob.bin"), [0u8, 1, 2, 0, 255]).expect("write");
    run_git(&repo, &["add", "blob.bin"]);
    let status = git().status(&repo, 100).expect("status");
    let lines = entry(&status.entries, "blob.bin")
        .staged_lines
        .expect("counts");
    assert!(lines.binary);
    assert_eq!((lines.added, lines.removed), (0, 0));
}

#[test]
fn a_rename_names_its_original_path() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["mv", "README.md", "READ ME.md"]);
    let status = git().status(&repo, 100).expect("status");
    let renamed = entry(&status.entries, "READ ME.md");
    assert_eq!(renamed.staged, Some(ChangeKind::Renamed));
    assert_eq!(renamed.original_path.as_deref(), Some("README.md"));
}

#[test]
fn files_staged_before_the_first_commit_have_line_counts() {
    let dir = tempfile::tempdir().expect("temp dir");
    run_git(dir.path(), &["init", "-q"]);
    std::fs::write(dir.path().join("first.txt"), "one\ntwo\n").expect("write");
    run_git(dir.path(), &["add", "first.txt"]);
    let status = git().status(dir.path(), 100).expect("status");
    assert_eq!(status.branch.oid, None);
    let first = entry(&status.entries, "first.txt");
    assert_eq!(first.staged, Some(ChangeKind::Added));
    assert_eq!(
        first.staged_lines.map(|l| (l.added, l.removed)),
        Some((2, 0))
    );
}

#[test]
fn a_merge_conflict_is_listed_with_the_operation() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
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
        .expect("run merge");
    assert!(!merge.status.success(), "the merge should conflict");

    let status = git().status(&repo, 100).expect("status");
    assert_eq!(status.operation, Some(GitOperation::Merge));
    let readme = entry(&status.entries, "README.md");
    assert_eq!(readme.conflict, Some(ConflictKind::BothModified));
    assert_eq!(status.totals.conflicted, 1);
}

#[test]
fn ahead_and_behind_count_against_the_upstream() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let remote = fixture.sibling("remote.git");
    run_git(
        &repo,
        &["clone", "-q", "--bare", text(&repo), text(&remote)],
    );
    run_git(&repo, &["remote", "add", "origin", text(&remote)]);
    run_git(&repo, &["fetch", "-q", "origin"]);
    run_git(&repo, &["branch", "-q", "--set-upstream-to=origin/main"]);
    commit_file(&repo, "local.txt", "x\n", "local");

    let status = git().status(&repo, 100).expect("status");
    assert_eq!(status.branch.upstream.as_deref(), Some("origin/main"));
    assert_eq!(
        (status.branch.ahead, status.branch.behind),
        (Some(1), Some(0))
    );
    assert!(!status.branch.upstream_gone);

    run_git(&repo, &["update-ref", "-d", "refs/remotes/origin/main"]);
    let status = git().status(&repo, 100).expect("status");
    assert!(status.branch.upstream_gone);
    assert_eq!(status.branch.ahead, None);
}

#[test]
fn a_status_read_does_not_rewrite_the_index() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let index = repo.join(".git").join("index");
    // A touched file makes a normal status refresh the index's stat data.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    std::fs::write(repo.join("README.md"), "hello\n").expect("touch");
    let before = std::fs::metadata(&index)
        .and_then(|m| m.modified())
        .expect("mtime");
    git().status(&repo, 100).expect("status");
    let after = std::fs::metadata(&index)
        .and_then(|m| m.modified())
        .expect("mtime");
    assert_eq!(before, after, "status rewrote the index");
}
