//! `git.commit` over the wire: a commit, an amend, and refusals.

use massorca_protocol::codes;
use massorca_protocol::methods::git::commit::GitCommitParams;
use massorca_protocol::methods::git::index::GitPathsParams;

use super::common::remote_code;
use super::git_fixture::{GitService, git};

fn commit(message: Option<&str>, amend: bool) -> GitCommitParams {
    GitCommitParams {
        workspace: "feature".to_owned(),
        message: message.map(str::to_owned),
        amend,
        allow_published: false,
    }
}

#[tokio::test]
async fn staged_changes_are_committed_and_then_amended() {
    let service = GitService::start().await;
    let dir = service.dir();
    std::fs::write(dir.join("a.txt"), "a\n").expect("write");
    service
        .client
        .git_stage(&GitPathsParams {
            workspace: "feature".to_owned(),
            paths: vec!["a.txt".to_owned()],
            all: false,
        })
        .await
        .expect("stage");

    let committed = service
        .client
        .git_commit(&commit(Some("add a"), false))
        .await
        .expect("commit");
    assert_eq!(committed.branch.as_deref(), Some("feature"));
    assert_eq!(committed.id, git(&dir, &["rev-parse", "HEAD"]).trim());

    let amended = service
        .client
        .git_commit(&commit(Some("add the a file"), true))
        .await
        .expect("amend");
    assert_eq!(amended.replaced.as_deref(), Some(committed.id.as_str()));
    assert_eq!(
        git(&dir, &["log", "-1", "--format=%s"]).trim(),
        "add the a file"
    );
    service.stop().await;
}

#[tokio::test]
async fn an_empty_commit_and_an_empty_message_are_refused() {
    let service = GitService::start().await;
    let error = service
        .client
        .git_commit(&commit(Some("x"), false))
        .await
        .expect_err("nothing staged");
    assert_eq!(remote_code(&error), codes::GIT_NOTHING_TO_COMMIT);
    let error = service
        .client
        .git_commit(&commit(Some(""), false))
        .await
        .expect_err("empty message");
    assert_eq!(remote_code(&error), codes::GIT_COMMIT_MESSAGE_EMPTY);
    service.stop().await;
}
