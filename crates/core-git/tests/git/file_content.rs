//! `Git::file_content`: one file's bytes on the working tree, the index or a revision, and the paths it refuses.

use massorca_core_git::{FileContentError, Git, SideContent};
use massorca_protocol::codes;
use massorca_protocol::methods::git::diff::FileSide;

use crate::fixture::{Fixture, commit_file, rev, run_git};

fn git() -> Git {
    Git::discover().expect("git is installed")
}

fn bytes(text: &str) -> SideContent {
    SideContent::Bytes(text.as_bytes().to_vec())
}

fn revision(rev: &str) -> FileSide {
    FileSide::Revision {
        rev: rev.to_owned(),
    }
}

/// The code of a refusal; any other outcome fails the test.
fn refusal(result: Result<SideContent, FileContentError>) -> &'static str {
    match result {
        Err(FileContentError::Refused { code, .. }) => code,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[cfg(unix)]
fn link_file(target: &std::path::Path, link: &std::path::Path) {
    std::os::unix::fs::symlink(target, link).expect("create symlink");
}

#[test]
fn the_worktree_side_is_the_file_on_disk() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    std::fs::write(repo.join("README.md"), "edited\n").expect("write");

    let content = git()
        .file_content(&repo, &FileSide::Worktree, "README.md", 1 << 20)
        .expect("content");
    assert_eq!(content, bytes("edited\n"));
}

#[test]
fn the_index_side_is_what_is_staged_not_the_edit_after_it() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    std::fs::write(repo.join("README.md"), "staged\n").expect("write");
    run_git(&repo, &["add", "README.md"]);
    std::fs::write(repo.join("README.md"), "unstaged\n").expect("write");

    let index = git()
        .file_content(&repo, &FileSide::Index, "README.md", 1 << 20)
        .expect("index");
    assert_eq!(index, bytes("staged\n"));
    let worktree = git()
        .file_content(&repo, &FileSide::Worktree, "README.md", 1 << 20)
        .expect("worktree");
    assert_eq!(worktree, bytes("unstaged\n"));
}

#[test]
fn a_revision_side_is_the_file_as_that_revision_holds_it() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    commit_file(&repo, "README.md", "second\n", "second");

    let older = git()
        .file_content(&repo, &revision("HEAD~1"), "README.md", 1 << 20)
        .expect("older");
    assert_eq!(older, bytes("hello\n"));
    let head = git()
        .file_content(&repo, &revision("HEAD"), "README.md", 1 << 20)
        .expect("head");
    assert_eq!(head, bytes("second\n"));
}

#[test]
fn a_side_with_no_file_is_absent_and_not_an_error() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    std::fs::write(repo.join("new.txt"), "untracked\n").expect("write");
    let absent = |side: &FileSide, path: &str| {
        git()
            .file_content(&repo, side, path, 1 << 20)
            .expect("no error")
    };
    assert_eq!(
        absent(&FileSide::Index, "new.txt"),
        SideContent::Absent,
        "not staged"
    );
    assert_eq!(
        absent(&revision("HEAD"), "new.txt"),
        SideContent::Absent,
        "not committed"
    );
    assert_eq!(absent(&FileSide::Worktree, "new.txt"), bytes("untracked\n"));

    run_git(&repo, &["rm", "-q", "README.md"]);
    assert_eq!(absent(&FileSide::Index, "README.md"), SideContent::Absent);
    assert_eq!(
        absent(&FileSide::Worktree, "README.md"),
        SideContent::Absent
    );
    assert_eq!(absent(&revision("HEAD"), "README.md"), bytes("hello\n"));
}

#[test]
fn a_file_over_the_cap_reports_its_size_and_no_bytes() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    commit_file(&repo, "big.bin", &"7".repeat(100), "big");

    for side in [FileSide::Worktree, FileSide::Index, revision("HEAD")] {
        let over = git()
            .file_content(&repo, &side, "big.bin", 99)
            .expect("content");
        assert_eq!(over, SideContent::TooLarge { size: 100 }, "{side:?}");
        let fits = git()
            .file_content(&repo, &side, "big.bin", 100)
            .expect("content");
        assert_eq!(fits, bytes(&"7".repeat(100)), "{side:?}");
    }
}

#[test]
fn a_dash_led_revision_is_refused_and_never_read_as_an_option() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    // Read as an option, `--output` would create this file.
    let output = fixture.sibling("written.txt");
    let option = format!("--output={}", output.display());

    assert_eq!(
        refusal(git().file_content(&repo, &revision(&option), "README.md", 1 << 20)),
        codes::GIT_REVISION_NOT_FOUND
    );
    assert!(
        !output.exists(),
        "git must not have written the output file"
    );
}

#[test]
fn an_unknown_revision_is_revision_not_found() {
    let fixture = Fixture::new();
    let repo = fixture.repo();

    assert_eq!(
        refusal(git().file_content(&repo, &revision("no-such-branch"), "README.md", 1 << 20)),
        codes::GIT_REVISION_NOT_FOUND
    );
}

#[test]
fn a_directory_or_submodule_is_refused_on_every_side() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    std::fs::create_dir_all(repo.join("docs")).expect("create dir");
    commit_file(&repo, "docs/guide.md", "guide\n", "add guide");
    let commit = rev(&repo, "HEAD");
    run_git(
        &repo,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{commit},vendor"),
        ],
    );

    for side in [FileSide::Worktree, FileSide::Index, revision("HEAD")] {
        assert_eq!(
            refusal(git().file_content(&repo, &side, "docs", 1 << 20)),
            codes::GIT_PATH_IS_DIRECTORY,
            "{side:?}"
        );
    }
    assert_eq!(
        refusal(git().file_content(&repo, &FileSide::Index, "vendor", 1 << 20)),
        codes::GIT_PATH_IS_DIRECTORY,
        "a submodule is a directory"
    );
}

#[test]
fn a_path_that_leaves_the_worktree_is_refused_on_every_side() {
    let fixture = Fixture::new();
    let repo = fixture.repo();

    for path in ["../repo/README.md", "/etc/passwd", "docs/../../outside.txt"] {
        for side in [FileSide::Worktree, FileSide::Index, revision("HEAD")] {
            assert_eq!(
                refusal(git().file_content(&repo, &side, path, 1 << 20)),
                codes::GIT_PATH_OUTSIDE_WORKTREE,
                "{path} on {side:?}"
            );
        }
    }
}

#[test]
#[cfg(unix)]
fn a_symlink_that_leaves_the_worktree_is_refused() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let outside = fixture.sibling("outside.txt");
    std::fs::write(&outside, "secret\n").expect("write");
    link_file(&outside, &repo.join("link.txt"));

    assert_eq!(
        refusal(git().file_content(&repo, &FileSide::Worktree, "link.txt", 1 << 20)),
        codes::GIT_PATH_OUTSIDE_WORKTREE
    );
}

#[test]
#[cfg(unix)]
fn a_symlink_to_a_file_inside_the_worktree_is_followed() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    link_file(&repo.join("README.md"), &repo.join("alias.md"));

    let content = git()
        .file_content(&repo, &FileSide::Worktree, "alias.md", 1 << 20)
        .expect("content");
    assert_eq!(content, bytes("hello\n"));
}

#[test]
fn git_s_own_folder_is_not_a_workspace_file() {
    let fixture = Fixture::new();
    let repo = fixture.repo();

    assert_eq!(
        refusal(git().file_content(&repo, &FileSide::Worktree, ".git/config", 1 << 20)),
        codes::REQUEST_INVALID_PARAMS
    );
}

#[test]
fn a_path_conflicted_in_the_index_is_refused() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    run_git(&repo, &["checkout", "-q", "-b", "theirs"]);
    commit_file(&repo, "README.md", "theirs\n", "theirs");
    run_git(&repo, &["checkout", "-q", "main"]);
    commit_file(&repo, "README.md", "ours\n", "ours");
    // The merge stops at the conflict and leaves its stages in the index, so its failure is the setup.
    massorca_process::command("git")
        .args(["merge", "-q", "--no-edit", "theirs"])
        .current_dir(&repo)
        .output()
        .expect("spawn git");

    assert_eq!(
        refusal(git().file_content(&repo, &FileSide::Index, "README.md", 1 << 20)),
        codes::GIT_PATH_CONFLICTED
    );
}

#[test]
#[cfg(unix)]
fn a_link_into_git_s_own_folder_is_refused() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    link_file(&repo.join(".git").join("config"), &repo.join("config-link"));

    assert_eq!(
        refusal(git().file_content(&repo, &FileSide::Worktree, "config-link", 1 << 20)),
        codes::REQUEST_INVALID_PARAMS
    );
}
