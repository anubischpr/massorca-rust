//! `git.fetch`, `git.pull`, `git.push` and `git.abort` over the wire, against a local bare remote.

use massorca_protocol::codes;
use massorca_protocol::methods::git::GitWorkspaceRef;
use massorca_protocol::methods::git::remote::{
    GitFetchParams, GitPullParams, GitPulled, GitPushParams, PullMode, PushOutcome,
};

use super::common::remote_code;
use super::git_fixture::{GitService, git};

fn utf8(path: &std::path::Path) -> &str {
    path.to_str().expect("test paths are UTF-8")
}

#[tokio::test]
async fn push_creates_the_upstream_and_fetch_and_pull_follow_it() {
    let service = GitService::start().await;
    let dir = service.dir();
    let remote = service.home.path().join("remote.git");
    git(
        service.home.path(),
        &["init", "-q", "--bare", utf8(&remote)],
    );
    git(&dir, &["remote", "add", "origin", utf8(&remote)]);

    let pushed = service
        .client
        .git_push(&GitPushParams {
            workspace: "feature".to_owned(),
            remote: None,
            force_with_lease: None,
            timeout_ms: None,
        })
        .await
        .expect("push");
    assert_eq!(pushed.outcome, PushOutcome::Created);
    assert!(pushed.upstream_set);
    assert_eq!(pushed.remote_branch, "origin/feature");

    let fetched = service
        .client
        .git_fetch(&GitFetchParams {
            workspace: "feature".to_owned(),
            remote: None,
            all: false,
            timeout_ms: Some(30_000),
        })
        .await
        .expect("fetch");
    assert!(fetched.updated.is_empty(), "{fetched:?}");

    let pulled = service
        .client
        .git_pull(&GitPullParams {
            workspace: "feature".to_owned(),
            mode: PullMode::FfOnly,
            timeout_ms: None,
        })
        .await
        .expect("pull");
    assert_eq!(pulled, GitPulled::UpToDate);
    service.stop().await;
}

#[tokio::test]
async fn pull_without_upstream_and_abort_without_operation_are_refused() {
    let service = GitService::start().await;
    let error = service
        .client
        .git_pull(&GitPullParams {
            workspace: "feature".to_owned(),
            mode: PullMode::FfOnly,
            timeout_ms: None,
        })
        .await
        .expect_err("no upstream");
    assert_eq!(remote_code(&error), codes::GIT_NO_UPSTREAM);
    let error = service
        .client
        .git_abort(&GitWorkspaceRef {
            workspace: "feature".to_owned(),
        })
        .await
        .expect_err("nothing in progress");
    assert_eq!(remote_code(&error), codes::GIT_NO_OPERATION);
    service.stop().await;
}

#[tokio::test]
async fn a_missing_remote_repository_is_a_typed_error() {
    let service = GitService::start().await;
    let dir = service.dir();
    let gone = service.home.path().join("gone.git");
    git(&dir, &["remote", "add", "origin", utf8(&gone)]);
    let error = service
        .client
        .git_fetch(&GitFetchParams {
            workspace: "feature".to_owned(),
            remote: Some("origin".to_owned()),
            all: false,
            timeout_ms: None,
        })
        .await
        .expect_err("missing");
    assert_eq!(remote_code(&error), codes::GIT_REMOTE_NOT_FOUND);
    service.stop().await;
}
