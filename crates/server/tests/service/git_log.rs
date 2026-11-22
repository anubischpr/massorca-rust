//! `git.log` over the wire: paging with cursors, revisions, and refusals.

use massorca_protocol::codes;
use massorca_protocol::methods::git::log::GitLogParams;

use super::common::remote_code;
use super::git_fixture::{GitService, git};

fn params(revision: Option<&str>, limit: Option<u32>, cursor: Option<String>) -> GitLogParams {
    GitLogParams {
        workspace: "feature".to_owned(),
        revision: revision.map(str::to_owned),
        limit,
        cursor,
    }
}

#[tokio::test]
async fn history_pages_follow_the_cursor_to_the_end() {
    let service = GitService::start().await;
    let dir = service.dir();
    for n in 0..3 {
        std::fs::write(dir.join("n.txt"), format!("{n}\n")).expect("write");
        git(&dir, &["add", "n.txt"]);
        git(&dir, &["commit", "-q", "-m", &format!("commit {n}")]);
    }
    let mut subjects = Vec::new();
    let mut cursor = None;
    loop {
        let page = service
            .client
            .git_log(&params(None, Some(2), cursor))
            .await
            .expect("log");
        subjects.extend(page.commits.into_iter().map(|commit| commit.subject));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(subjects, ["commit 2", "commit 1", "commit 0", "initial"]);

    let main = service
        .client
        .git_log(&params(Some("main"), None, None))
        .await
        .expect("log of main");
    assert_eq!(main.commits.len(), 1);
    service.stop().await;
}

#[tokio::test]
async fn an_unknown_revision_and_a_forged_cursor_are_refused() {
    let service = GitService::start().await;
    let error = service
        .client
        .git_log(&params(Some("no-such-branch"), None, None))
        .await
        .expect_err("unknown revision");
    assert_eq!(remote_code(&error), codes::GIT_REVISION_NOT_FOUND);

    let error = service
        .client
        .git_log(&params(None, None, Some("--all:0".to_owned())))
        .await
        .expect_err("forged cursor");
    assert_eq!(remote_code(&error), codes::REQUEST_INVALID_PARAMS);
    service.stop().await;
}
