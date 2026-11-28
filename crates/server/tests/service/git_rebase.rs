//! `git.rebase_base` over the wire (decision 0181): a worktree's branch onto its base, the typed outcomes, the refusals,
//! and the refresh from a local bare remote.

use std::time::Duration;

use massorca_client::Subscription;
use massorca_protocol::codes;
use massorca_protocol::methods::git::GitWorkspaceRef;
use massorca_protocol::methods::git::rebase::{GitRebaseBaseParams, GitRebased};
use massorca_protocol::methods::git::watch::{GitWatchEvent, WatchKind};
use massorca_protocol::methods::workspaces::{self, Workspace, WorkspaceCreateParams};
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

/// Items until every kind in `wanted` has been reported, whichever bursts carry them.
async fn until_kinds(stream: &mut Subscription, wanted: &[WatchKind]) {
    let mut seen = Vec::new();
    while !wanted.iter().all(|kind| seen.contains(kind)) {
        seen.extend(next(stream).await.kinds);
    }
}

fn utf8(path: &std::path::Path) -> &str {
    path.to_str().expect("test paths are UTF-8")
}

fn params(workspace: &str, refresh: bool) -> GitRebaseBaseParams {
    GitRebaseBaseParams {
        workspace: workspace.to_owned(),
        refresh,
    }
}

#[tokio::test]
async fn rebase_base_moves_the_branch_onto_the_advanced_base_then_is_up_to_date() {
    let service = GitService::start().await;
    let dir = service.dir();
    let repo = service.home.path().join("repo");
    std::fs::write(dir.join("feature.txt"), "feature\n").expect("write feature file");
    git(&dir, &["add", "feature.txt"]);
    git(&dir, &["commit", "-q", "-m", "feature work"]);
    std::fs::write(repo.join("main.txt"), "main\n").expect("write main file");
    git(&repo, &["add", "main.txt"]);
    git(&repo, &["commit", "-q", "-m", "main moves on"]);
    let base_tip = git(&repo, &["rev-parse", "main"]).trim().to_owned();

    let rebased = service
        .client
        .git_rebase_base(&params("feature", false))
        .await
        .expect("rebase runs");

    let head = git(&dir, &["rev-parse", "HEAD"]).trim().to_owned();
    assert_eq!(rebased, GitRebased::Rebased { head: head.clone() });
    assert_eq!(git(&dir, &["rev-parse", "HEAD~1"]).trim(), base_tip);
    let again = service
        .client
        .git_rebase_base(&params("feature", false))
        .await
        .expect("rebase runs again");
    assert_eq!(again, GitRebased::UpToDate);
    assert_eq!(git(&dir, &["rev-parse", "HEAD"]).trim(), head);
    service.stop().await;
}

#[tokio::test]
async fn a_dirty_workspace_is_refused_and_a_folder_workspace_is_not_a_repository() {
    let service = GitService::start().await;
    std::fs::write(service.dir().join("loose.txt"), "not committed\n").expect("write file");
    let dirty = service
        .client
        .git_rebase_base(&params("feature", false))
        .await
        .expect_err("a dirty worktree is refused");
    assert_eq!(remote_code(&dirty), codes::WORKSPACE_DIRTY);

    let folder = service.folder_workspace().await;
    let not_git = service
        .client
        .git_rebase_base(&params(&folder.name, false))
        .await
        .expect_err("a folder is refused");
    assert_eq!(remote_code(&not_git), codes::GIT_NOT_A_REPOSITORY);
    service.stop().await;
}

#[tokio::test]
async fn a_conflicting_rebase_lists_the_paths_and_leaves_the_worktree_as_it_was() {
    let service = GitService::start().await;
    let dir = service.dir();
    let repo = service.home.path().join("repo");
    std::fs::write(dir.join("README.md"), "from feature\n").expect("write README");
    git(&dir, &["commit", "-q", "-am", "feature change"]);
    std::fs::write(repo.join("README.md"), "from main\n").expect("write README");
    git(&repo, &["commit", "-q", "-am", "main change"]);
    let head_before = git(&dir, &["rev-parse", "HEAD"]).trim().to_owned();

    let outcome = service
        .client
        .git_rebase_base(&params("feature", false))
        .await
        .expect("the conflict is an outcome, not an error");

    assert_eq!(
        outcome,
        GitRebased::Conflict {
            paths: vec!["README.md".to_owned()]
        }
    );
    assert_eq!(git(&dir, &["rev-parse", "HEAD"]).trim(), head_before);
    assert_eq!(
        std::fs::read_to_string(dir.join("README.md")).expect("read README"),
        "from feature\n"
    );
    service.stop().await;
}

#[tokio::test]
async fn refresh_fetches_the_base_from_its_bare_remote_before_the_rebase() {
    let service = GitService::start().await;
    let repo = service.home.path().join("repo");
    let remote = service.home.path().join("remote.git");
    git(
        service.home.path(),
        &["init", "-q", "--bare", utf8(&remote)],
    );
    git(&repo, &["remote", "add", "origin", utf8(&remote)]);
    // Full ref names, so the pushes and the clone below do not depend on the machine's default branch name.
    git(
        &repo,
        &["push", "-q", "origin", "refs/heads/main:refs/heads/main"],
    );
    let project = service.workspace.project_id.clone();
    let later: Workspace = service
        .client
        .call(
            workspaces::CREATE,
            &WorkspaceCreateParams {
                project,
                name: "later".to_owned(),
                branch: None,
                base: Some("origin/main".to_owned()),
                existing_branch: false,
                refresh_base: false,
                path: None,
                request_id: None,
                no_setup: false,
                unique_name: false,
                name_generated: false,
                in_place: false,
            },
        )
        .await
        .expect("create a workspace from origin/main");
    // Another clone moves the remote's main on.
    let other = service.home.path().join("other");
    git(
        service.home.path(),
        &["clone", "-q", "-b", "main", utf8(&remote), utf8(&other)],
    );
    git(&other, &["config", "user.name", "t"]);
    git(&other, &["config", "user.email", "t@example.invalid"]);
    git(&other, &["config", "commit.gpgsign", "false"]);
    std::fs::write(other.join("remote.txt"), "remote\n").expect("write file");
    git(&other, &["add", "remote.txt"]);
    git(&other, &["commit", "-q", "-m", "remote moves on"]);
    git(
        &other,
        &["push", "-q", "origin", "refs/heads/main:refs/heads/main"],
    );
    let remote_tip = git(&other, &["rev-parse", "HEAD"]).trim().to_owned();

    let without_refresh = service
        .client
        .git_rebase_base(&params(&later.name, false))
        .await
        .expect("rebase without refresh");
    assert_eq!(without_refresh, GitRebased::UpToDate);

    let with_refresh = service
        .client
        .git_rebase_base(&params(&later.name, true))
        .await
        .expect("rebase with refresh");
    assert!(
        matches!(with_refresh, GitRebased::Rebased { .. }),
        "{with_refresh:?}"
    );
    assert_eq!(
        git(std::path::Path::new(&later.path), &["rev-parse", "HEAD"]).trim(),
        remote_tip
    );
    service.stop().await;
}

/// No event is published for the rebase itself: the `git.watch` stream reports the HEAD and ref changes it makes.
#[tokio::test]
async fn a_rebase_reaches_the_git_watch_stream_as_head_and_refs_changes() {
    let service = GitService::start().await;
    let dir = service.dir();
    let repo = service.home.path().join("repo");
    std::fs::write(dir.join("feature.txt"), "feature\n").expect("write feature file");
    git(&dir, &["add", "feature.txt"]);
    git(&dir, &["commit", "-q", "-m", "feature work"]);
    std::fs::write(repo.join("main.txt"), "main\n").expect("write main file");
    git(&repo, &["add", "main.txt"]);
    git(&repo, &["commit", "-q", "-m", "main moves on"]);
    let mut stream = service
        .client
        .git_watch(&GitWorkspaceRef {
            workspace: "feature".to_owned(),
        })
        .await
        .expect("watch");
    let ready = next(&mut stream).await;
    assert!(ready.ready && !ready.degraded, "{ready:?}");
    // `ready` is sent once the watchers are armed, so the rebase needs no wait before it.

    let rebased = service
        .client
        .git_rebase_base(&params("feature", false))
        .await
        .expect("rebase runs");
    assert!(matches!(rebased, GitRebased::Rebased { .. }), "{rebased:?}");

    until_kinds(&mut stream, &[WatchKind::Head, WatchKind::Refs]).await;
    service.stop().await;
}

#[tokio::test]
async fn an_in_place_checkout_is_refused_as_the_main_worktree() {
    let service = GitService::start().await;
    let in_place: Workspace = service
        .client
        .call(
            workspaces::CREATE,
            &WorkspaceCreateParams {
                project: service.workspace.project_id.clone(),
                name: String::new(),
                branch: None,
                base: None,
                existing_branch: false,
                refresh_base: false,
                path: None,
                request_id: None,
                no_setup: false,
                unique_name: false,
                name_generated: false,
                in_place: true,
            },
        )
        .await
        .expect("register the project's checkout in place");
    let refused = service
        .client
        .git_rebase_base(&params(&in_place.id, false))
        .await
        .expect_err("an in-place checkout is never rebased");
    assert_eq!(remote_code(&refused), codes::WORKSPACE_MAIN_WORKTREE);
    service.stop().await;
}
