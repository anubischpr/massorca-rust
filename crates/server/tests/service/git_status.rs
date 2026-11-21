//! `git.status` over the wire: a worktree workspace's changes, the limit, and the refusals.

use massorca_protocol::codes;
use massorca_protocol::methods::git::ChangeKind;
use massorca_protocol::methods::git::status::GitStatusParams;

use super::common::remote_code;
use super::git_fixture::GitService;

fn params(workspace: &str, limit: Option<u32>) -> GitStatusParams {
    GitStatusParams {
        workspace: workspace.to_owned(),
        limit,
    }
}

#[tokio::test]
async fn status_lists_the_workspace_changes_by_name_id_and_path() {
    let service = GitService::start().await;
    let dir = service.dir();
    std::fs::write(dir.join("README.md"), "hello\nmore\n").expect("write");
    std::fs::write(dir.join("new.txt"), "x\n").expect("write");

    for selector in [
        "feature".to_owned(),
        service.workspace.id.clone(),
        service.workspace.path.clone(),
    ] {
        let status = service
            .client
            .git_status(&params(&selector, None))
            .await
            .expect("status");
        assert_eq!(status.branch.head.as_deref(), Some("feature"));
        let paths: Vec<_> = status.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["README.md", "new.txt"]);
        assert_eq!(status.entries[0].unstaged, Some(ChangeKind::Modified));
        assert!(status.entries[1].untracked);
    }
    service.stop().await;
}

#[tokio::test]
async fn the_limit_truncates_entries_and_keeps_totals() {
    let service = GitService::start().await;
    for name in ["a", "b", "c"] {
        std::fs::write(service.dir().join(name), "x\n").expect("write");
    }
    let status = service
        .client
        .git_status(&params("feature", Some(2)))
        .await
        .expect("status");
    assert!(status.truncated);
    assert_eq!(status.entries.len(), 2);
    assert_eq!(status.totals.untracked, 3);

    // A limit of zero is raised to one rather than refused.
    let status = service
        .client
        .git_status(&params("feature", Some(0)))
        .await
        .expect("status");
    assert_eq!(status.entries.len(), 1);
    service.stop().await;
}

#[tokio::test]
async fn a_folder_workspace_is_not_a_repository() {
    let service = GitService::start().await;
    let folder = service.folder_workspace().await;
    let error = service
        .client
        .git_status(&params(&folder.id, None))
        .await
        .expect_err("a folder has no git status");
    assert_eq!(remote_code(&error), codes::GIT_NOT_A_REPOSITORY);
    service.stop().await;
}

#[tokio::test]
async fn an_unknown_workspace_and_a_missing_directory_are_refused() {
    let service = GitService::start().await;
    let error = service
        .client
        .git_status(&params("nope", None))
        .await
        .expect_err("no such workspace");
    assert_eq!(remote_code(&error), codes::WORKSPACE_NOT_FOUND);

    std::fs::remove_dir_all(service.dir()).expect("remove the worktree directory");
    let error = service
        .client
        .git_status(&params("feature", None))
        .await
        .expect_err("the directory is gone");
    assert_eq!(remote_code(&error), codes::PATH_NOT_FOUND);
    service.stop().await;
}
