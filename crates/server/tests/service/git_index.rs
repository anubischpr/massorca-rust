//! `git.stage`, `git.unstage` and `git.discard` over the wire, with refusals that carry their paths.

use massorca_client::ClientError;
use massorca_protocol::codes;
use massorca_protocol::methods::git::ChangeKind;
use massorca_protocol::methods::git::index::{GitDiscardParams, GitPathsParams};
use massorca_protocol::methods::git::status::GitStatusParams;
use serde_json::json;

use super::common::remote_code;
use super::git_fixture::GitService;

fn paths(list: &[&str]) -> GitPathsParams {
    GitPathsParams {
        workspace: "feature".to_owned(),
        paths: list.iter().map(|path| (*path).to_owned()).collect(),
        all: false,
    }
}

fn discard(list: &[&str]) -> GitDiscardParams {
    GitDiscardParams {
        workspace: "feature".to_owned(),
        paths: list.iter().map(|path| (*path).to_owned()).collect(),
        all: false,
        delete_untracked: false,
        include_staged: false,
        dry_run: false,
    }
}

#[tokio::test]
async fn stage_then_unstage_moves_a_file_between_the_lists() {
    let service = GitService::start().await;
    std::fs::write(service.dir().join("README.md"), "edited\n").expect("write");
    service
        .client
        .git_stage(&paths(&["README.md"]))
        .await
        .expect("stage");
    let status = service
        .client
        .git_status(&GitStatusParams {
            workspace: "feature".to_owned(),
            limit: None,
        })
        .await
        .expect("status");
    assert_eq!(status.entries[0].staged, Some(ChangeKind::Modified));

    service
        .client
        .git_unstage(&paths(&["README.md"]))
        .await
        .expect("unstage");
    let status = service
        .client
        .git_status(&GitStatusParams {
            workspace: "feature".to_owned(),
            limit: None,
        })
        .await
        .expect("status");
    assert_eq!(status.entries[0].staged, None);
    assert_eq!(status.entries[0].unstaged, Some(ChangeKind::Modified));
    service.stop().await;
}

#[tokio::test]
async fn a_discard_refusal_lists_the_files_and_changes_nothing() {
    let service = GitService::start().await;
    let dir = service.dir();
    std::fs::write(dir.join("new.txt"), "work\n").expect("write");
    let error = service
        .client
        .git_discard(&discard(&["new.txt"]))
        .await
        .expect_err("refused");
    assert_eq!(remote_code(&error), codes::GIT_DISCARD_UNTRACKED);
    match error {
        ClientError::Remote(body) => assert_eq!(body.data, Some(json!({ "paths": ["new.txt"] }))),
        other => panic!("{other:?}"),
    }
    assert!(dir.join("new.txt").exists());

    let done = service
        .client
        .git_discard(&GitDiscardParams {
            delete_untracked: true,
            ..discard(&["new.txt"])
        })
        .await
        .expect("discard");
    assert_eq!(done.deleted, ["new.txt"]);
    assert!(!dir.join("new.txt").exists());
    service.stop().await;
}

#[tokio::test]
async fn paths_and_all_are_exclusive_and_paths_stay_inside() {
    let service = GitService::start().await;
    let error = service
        .client
        .git_stage(&paths(&[]))
        .await
        .expect_err("nothing named");
    assert_eq!(remote_code(&error), codes::GIT_PATHS_REQUIRED);
    let error = service
        .client
        .git_stage(&GitPathsParams {
            all: true,
            ..paths(&["README.md"])
        })
        .await
        .expect_err("both named");
    assert_eq!(remote_code(&error), codes::GIT_PATHS_REQUIRED);
    let error = service
        .client
        .git_discard(&discard(&["../repo/README.md"]))
        .await
        .expect_err("outside");
    assert_eq!(remote_code(&error), codes::GIT_PATH_OUTSIDE_WORKTREE);
    service.stop().await;
}

#[tokio::test]
async fn concurrent_stages_on_one_workspace_never_collide_on_the_index_lock() {
    let service = GitService::start().await;
    let names: Vec<String> = (0..12).map(|n| format!("f{n}.txt")).collect();
    for name in &names {
        std::fs::write(service.dir().join(name), "x\n").expect("write");
    }
    let mut calls = tokio::task::JoinSet::new();
    for name in &names {
        let client = service.client.clone();
        let params = paths(&[name.as_str()]);
        calls.spawn(async move { client.git_stage(&params).await });
    }
    while let Some(outcome) = calls.join_next().await {
        outcome.expect("join").expect("every stage succeeds");
    }
    service.stop().await;
}
