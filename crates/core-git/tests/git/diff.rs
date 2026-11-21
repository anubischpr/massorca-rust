//! `Git::diff` and `Git::diff_file` against real repositories.

use massorca_core_git::Git;
use massorca_protocol::methods::git::ChangeKind;
use massorca_protocol::methods::git::diff::{DiffEntry, DiffTarget};

use crate::fixture::{Fixture, commit_file, rev, run_git};

fn git() -> Git {
    Git::discover().expect("git is installed")
}

fn file<'a>(files: &'a [DiffEntry], path: &str) -> &'a DiffEntry {
    files
        .iter()
        .find(|file| file.path == path)
        .unwrap_or_else(|| panic!("no diff entry for {path}: {files:?}"))
}

#[test]
fn the_worktree_diff_counts_lines_and_skips_files_only_touched() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    commit_file(&repo, "same.txt", "same\n", "add same");
    std::fs::write(repo.join("README.md"), "hello\nworld\n").expect("write");
    // Rewriting identical content changes the stat data only.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    std::fs::write(repo.join("same.txt"), "same\n").expect("touch");

    let diff = git().diff(&repo, &DiffTarget::Worktree, 100).expect("diff");
    let paths: Vec<_> = diff.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["README.md"]);
    assert_eq!(diff.files[0].change, ChangeKind::Modified);
    assert_eq!(
        (diff.totals.files, diff.totals.added, diff.totals.removed),
        (1, 1, 0)
    );
}

#[test]
fn a_staged_rename_is_listed_and_its_patch_shows_the_rename() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["mv", "README.md", "docs.md"]);

    let diff = git().diff(&repo, &DiffTarget::Staged, 100).expect("diff");
    let renamed = file(&diff.files, "docs.md");
    assert_eq!(renamed.change, ChangeKind::Renamed);
    assert_eq!(renamed.original_path.as_deref(), Some("README.md"));

    let patch = git()
        .diff_file(
            &repo,
            &DiffTarget::Staged,
            "docs.md",
            Some("README.md"),
            1 << 20,
        )
        .expect("patch");
    let text = patch.patch.expect("a patch");
    assert!(text.contains("rename from README.md"), "{text}");
    assert!(text.contains("rename to docs.md"), "{text}");
}

#[test]
fn the_head_diff_joins_staged_and_unstaged_changes() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    std::fs::write(repo.join("README.md"), "hello\nstaged\n").expect("write");
    run_git(&repo, &["add", "README.md"]);
    std::fs::write(repo.join("README.md"), "hello\nstaged\nunstaged\n").expect("write");

    let diff = git().diff(&repo, &DiffTarget::Head, 100).expect("diff");
    assert_eq!(file(&diff.files, "README.md").lines.added, 2);
    let patch = git()
        .diff_file(&repo, &DiffTarget::Head, "README.md", None, 1 << 20)
        .expect("patch")
        .patch
        .expect("a patch");
    assert!(patch.contains("+staged\n+unstaged\n"), "{patch}");
}

#[test]
fn a_range_compares_two_commits() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let first = rev(&repo, "HEAD");
    commit_file(&repo, "added.txt", "a\nb\n", "add");
    let target = DiffTarget::Range {
        from: first,
        to: None,
    };
    let diff = git().diff(&repo, &target, 100).expect("diff");
    let added = file(&diff.files, "added.txt");
    assert_eq!(added.change, ChangeKind::Added);
    assert_eq!(added.lines.added, 2);
}

#[test]
fn a_binary_file_returns_sizes_and_no_text() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    std::fs::write(repo.join("blob.bin"), [0u8; 10]).expect("write");
    run_git(&repo, &["add", "blob.bin"]);
    run_git(&repo, &["commit", "-q", "-m", "blob"]);
    std::fs::write(repo.join("blob.bin"), [0u8; 25]).expect("write");

    let patch = git()
        .diff_file(&repo, &DiffTarget::Worktree, "blob.bin", None, 1 << 20)
        .expect("patch");
    assert!(patch.binary);
    assert_eq!(patch.patch, None);
    assert_eq!((patch.old_size, patch.new_size), (Some(10), Some(25)));
}

#[test]
fn a_patch_past_the_cap_is_cut_at_a_line_break() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let big: String = (0..20_000).map(|n| format!("line {n}\n")).collect();
    std::fs::write(repo.join("README.md"), big).expect("write");

    let patch = git()
        .diff_file(&repo, &DiffTarget::Worktree, "README.md", None, 4096)
        .expect("patch");
    assert!(patch.truncated);
    let text = patch.patch.expect("a patch");
    assert!(text.len() <= 4096, "{} bytes", text.len());
    assert!(text.ends_with('\n'));
    assert!(text.starts_with("diff --git"));
}

#[test]
fn an_untracked_file_is_shown_as_added_and_binary_ones_are_marked() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    std::fs::write(repo.join("new file.txt"), "one\ntwo\n").expect("write");
    std::fs::write(repo.join("new.bin"), [1u8, 0, 2]).expect("write");

    let patch = git()
        .diff_file(&repo, &DiffTarget::Worktree, "new file.txt", None, 1 << 20)
        .expect("patch");
    assert_eq!(
        patch.patch.as_deref(),
        Some("--- /dev/null\n+++ b/new file.txt\n@@ -0,0 +1,2 @@\n+one\n+two\n")
    );
    let binary = git()
        .diff_file(&repo, &DiffTarget::Head, "new.bin", None, 1 << 20)
        .expect("patch");
    assert!(binary.binary);
    assert_eq!(binary.new_size, Some(3));
}

#[test]
fn an_unchanged_path_has_no_patch() {
    let fixture = Fixture::new();
    let patch = git()
        .diff_file(
            &fixture.repo(),
            &DiffTarget::Worktree,
            "README.md",
            None,
            1 << 20,
        )
        .expect("patch");
    assert_eq!(patch.patch, None);
    assert!(!patch.binary && !patch.truncated);
}

#[test]
fn staged_files_before_the_first_commit_diff_against_the_empty_tree() {
    let dir = tempfile::tempdir().expect("temp dir");
    run_git(dir.path(), &["init", "-q"]);
    std::fs::write(dir.path().join("first.txt"), "x\n").expect("write");
    run_git(dir.path(), &["add", "first.txt"]);
    let diff = git()
        .diff(dir.path(), &DiffTarget::Staged, 100)
        .expect("diff");
    assert_eq!(file(&diff.files, "first.txt").change, ChangeKind::Added);
}

#[test]
fn a_mode_change_is_listed_even_without_line_changes() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["update-index", "--chmod=+x", "README.md"]);
    let diff = git().diff(&repo, &DiffTarget::Staged, 100).expect("diff");
    let readme = file(&diff.files, "README.md");
    assert_eq!(readme.change, ChangeKind::Modified);
    assert_eq!((readme.lines.added, readme.lines.removed), (0, 0));
}

#[test]
fn glob_characters_in_a_path_match_only_that_file() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    commit_file(&repo, "a[b].txt", "star\n", "bracket");
    commit_file(&repo, "ab.txt", "plain\n", "plain");
    std::fs::write(repo.join("a[b].txt"), "star\nchanged\n").expect("write");
    std::fs::write(repo.join("ab.txt"), "plain\nchanged\n").expect("write");
    let patch = git()
        .diff_file(&repo, &DiffTarget::Worktree, "a[b].txt", None, 1 << 20)
        .expect("patch")
        .patch
        .expect("a patch");
    assert!(patch.contains("a/a[b].txt"), "{patch}");
    assert!(!patch.contains("ab.txt"), "{patch}");
}
