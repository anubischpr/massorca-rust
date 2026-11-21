//! `git.watch` over the wire: the ready item, change items, two subscribers on one workspace, and refusals.

use std::time::Duration;

use massorca_client::Subscription;
use massorca_protocol::codes;
use massorca_protocol::methods::git::GitWorkspaceRef;
use massorca_protocol::methods::git::watch::{GitWatchEvent, WatchKind};
use tokio::time::timeout;

use super::common::remote_code;
use super::git_fixture::{GitService, git};

async fn next(stream: &mut Subscription) -> GitWatchEvent {
    timeout(Duration::from_secs(15), stream.next_as::<GitWatchEvent>())
        .await
        .expect("an item arrives")
        .expect("the stream is open")
        .expect("the item decodes")
}

/// Items until one names `kind`, within the overall wait of `next`.
async fn until_kind(stream: &mut Subscription, kind: WatchKind) {
    loop {
        if next(stream).await.kinds.contains(&kind) {
            return;
        }
    }
}

fn workspace() -> GitWorkspaceRef {
    GitWorkspaceRef {
        workspace: "feature".to_owned(),
    }
}

#[tokio::test]
async fn subscribers_get_ready_then_the_kinds_that_changed() {
    let service = GitService::start().await;
    let dir = service.dir();
    let mut first = service.client.git_watch(&workspace()).await.expect("watch");
    let mut second = service.client.git_watch(&workspace()).await.expect("watch");
    for stream in [&mut first, &mut second] {
        let ready = next(stream).await;
        assert!(
            ready.ready && ready.kinds.is_empty() && !ready.degraded,
            "{ready:?}"
        );
    }
    tokio::time::sleep(Duration::from_millis(300)).await;

    std::fs::write(dir.join("new.txt"), "x").expect("write");
    until_kind(&mut first, WatchKind::Worktree).await;
    until_kind(&mut second, WatchKind::Worktree).await;

    git(&dir, &["add", "new.txt"]);
    until_kind(&mut first, WatchKind::Index).await;
    git(&dir, &["commit", "-q", "-m", "new"]);
    until_kind(&mut first, WatchKind::Refs).await;
    service.stop().await;
}

#[tokio::test]
async fn a_folder_workspace_cannot_be_watched() {
    let service = GitService::start().await;
    let folder = service.folder_workspace().await;
    let error = service
        .client
        .git_watch(&GitWorkspaceRef {
            workspace: folder.id,
        })
        .await
        .expect_err("a folder has no git state");
    assert_eq!(remote_code(&error), codes::GIT_NOT_A_REPOSITORY);
    service.stop().await;
}
