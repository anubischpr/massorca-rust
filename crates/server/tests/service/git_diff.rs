//! `git.diff` and `git.diff_file` over the wire, and their refusals.

use massorca_protocol::codes;
use massorca_protocol::methods::git::diff::{DiffTarget, GitDiffFileParams, GitDiffParams};

use super::common::remote_code;
use super::git_fixture::{GitService, git};

fn file_params(target: DiffTarget, path: &str) -> GitDiffFileParams {
    GitDiffFileParams {
        workspace: "feature".to_owned(),
        target,
        path: path.to_owned(),
        original_path: None,
        max_bytes: None,
    }
}

#[tokio::test]
async fn the_diff_lists_files_and_the_patch_shows_one() {
    let service = GitService::start().await;
    std::fs::write(service.dir().join("README.md"), "hello\nworld\n").expect("write");

    let diff = service
        .client
        .git_diff(&GitDiffParams {
            workspace: "feature".to_owned(),
            target: DiffTarget::Worktree,
            limit: None,
        })
        .await
        .expect("diff");
    assert_eq!(diff.files.len(), 1);
    assert_eq!(diff.files[0].path, "README.md");
    assert_eq!(diff.files[0].lines.added, 1);

    let patch = service
        .client
        .git_diff_file(&file_params(DiffTarget::Worktree, "README.md"))
        .await
        .expect("patch");
    assert!(patch.patch.expect("text").contains("+world"));

    let capped = service
        .client
        .git_diff_file(&GitDiffFileParams {
            max_bytes: Some(10),
            ..file_params(DiffTarget::Worktree, "README.md")
        })
        .await
        .expect("patch");
    assert!(capped.truncated);
    service.stop().await;
}

#[tokio::test]
async fn a_range_between_commits_resolves_its_revisions() {
    let service = GitService::start().await;
    let dir = service.dir();
    std::fs::write(dir.join("new.txt"), "x\n").expect("write");
    git(&dir, &["add", "new.txt"]);
    git(&dir, &["commit", "-q", "-m", "new"]);
    let diff = service
        .client
        .git_diff(&GitDiffParams {
            workspace: "feature".to_owned(),
            target: DiffTarget::Range {
                from: "main".to_owned(),
                to: Some("feature".to_owned()),
            },
            limit: None,
        })
        .await
        .expect("diff");
    assert_eq!(diff.files[0].path, "new.txt");

    let error = service
        .client
        .git_diff(&GitDiffParams {
            workspace: "feature".to_owned(),
            target: DiffTarget::Range {
                from: "--output=/tmp/x".to_owned(),
                to: None,
            },
            limit: None,
        })
        .await
        .expect_err("an option is not a revision");
    assert_eq!(remote_code(&error), codes::GIT_REVISION_NOT_FOUND);
    service.stop().await;
}

#[tokio::test]
async fn paths_that_could_leave_the_workspace_are_refused() {
    let service = GitService::start().await;
    for path in ["../repo/README.md", "/etc/passwd", "a/../../b", "C:/x", ""] {
        let error = service
            .client
            .git_diff_file(&file_params(DiffTarget::Worktree, path))
            .await
            .expect_err("refused");
        assert_eq!(
            remote_code(&error),
            codes::GIT_PATH_OUTSIDE_WORKTREE,
            "{path}"
        );
    }
    service.stop().await;
}
