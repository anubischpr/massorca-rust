//! The binding's reads and streams driven from Rust as the app drives them (decision 0080, task A4): the workspaces, the
//! git status, and the git and agent watches, which reconnect when the host stops and returns.

mod common;

use std::future::Future;
use std::net::{Ipv4Addr, TcpListener};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use common::{app, offer, owner, start, stop, temp_home};
use massorca_client_ffi::{AgentEvent, AgentItem, GitEvent, Host, LinkState, MassOrca};
use massorca_protocol::codes;
use massorca_protocol::methods::projects::{self, Project, ProjectAddParams, ProjectKind};
use massorca_protocol::methods::workspaces::{self, Workspace, WorkspaceCreateParams};
use tokio::runtime::Runtime;
use tokio::time::timeout;

/// How long one read may take before the test calls it stuck; git alone can take seconds on a Windows runner.
const WAIT: Duration = Duration::from_secs(30);

/// Waits for `future` on the test's runtime, at most `WAIT`.
fn within<T>(test: &Runtime, future: impl Future<Output = T>) -> T {
    test.block_on(async { timeout(WAIT, future).await.expect("an answer in time") })
}

/// Pairs the app with the host under `home` as `office` and connects to it. The app is returned too: dropping it
/// stops the runtime that serves the connection.
fn connected(test: &Runtime, home: &Path) -> (Arc<MassOrca>, Arc<Host>) {
    let massorca = app();
    let text = test.block_on(offer(home));
    test.block_on(massorca.pair(text, "office".into()))
        .expect("paired");
    let host = test
        .block_on(massorca.connect("office".into()))
        .expect("connected");
    (massorca, host)
}

/// A folder project named `name` with one folder workspace of that name.
fn folder_workspace(test: &Runtime, home: &Path, name: &str) -> Workspace {
    let folder = home.join(name);
    std::fs::create_dir_all(&folder).expect("a folder");
    test.block_on(async {
        let client = owner(home).await;
        let project: Project = client
            .call(
                projects::ADD,
                &ProjectAddParams {
                    icon: None,
                    path: utf8(&folder),
                    name: Some(name.to_owned()),
                    kind: Some(ProjectKind::Folder),
                    workspaces_root: Some(utf8(&home.join(format!("ws-{name}")))),
                },
            )
            .await
            .expect("the folder project");
        client
            .call(
                workspaces::CREATE,
                &WorkspaceCreateParams {
                    project: project.id,
                    name: name.to_owned(),
                    branch: None,
                    base: None,
                    existing_branch: false,
                    refresh_base: false,
                    no_setup: false,
                    unique_name: false,
                    path: None,
                    request_id: None,
                    name_generated: false,
                },
            )
            .await
            .expect("the folder workspace")
    })
}

/// A git project with one empty commit, and a worktree workspace on a new branch. Each git call is slow to start on
/// some hosts, so the fixture makes as few as it can.
fn git_workspace(test: &Runtime, home: &Path) -> Workspace {
    let repo = home.join("repo");
    std::fs::create_dir_all(&repo).expect("a repository folder");
    git(&repo, &["init", "-q"]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "initial"]);
    test.block_on(async {
        let client = owner(home).await;
        let project: Project = client
            .call(
                projects::ADD,
                &ProjectAddParams {
                    icon: None,
                    path: utf8(&repo),
                    name: Some("repo".to_owned()),
                    kind: None,
                    workspaces_root: Some(utf8(&home.join("ws-repo"))),
                },
            )
            .await
            .expect("the git project");
        client
            .call(
                workspaces::CREATE,
                &WorkspaceCreateParams {
                    project: project.id,
                    name: "feature".to_owned(),
                    branch: None,
                    base: None,
                    existing_branch: false,
                    refresh_base: false,
                    no_setup: false,
                    unique_name: false,
                    path: None,
                    request_id: None,
                    name_generated: false,
                },
            )
            .await
            .expect("the worktree workspace")
    })
}

/// Runs git in `dir` with a fixed identity; panics when it fails.
fn git(dir: &Path, args: &[&str]) {
    let output = massorca_process::command("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn utf8(path: &Path) -> String {
    path.to_str().expect("test paths are UTF-8").to_owned()
}

#[test]
fn workspaces_lists_every_project_and_one_project_when_asked() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().unwrap();
    let running = test.block_on(start(&home, 0));
    let plain = folder_workspace(&test, &home, "plain");
    let other = folder_workspace(&test, &home, "other");
    let (_massorca, host) = connected(&test, &home);

    let all = within(&test, host.workspaces(None)).expect("listed");
    assert_eq!(all.len(), 2, "{all:?}");
    let listed = all
        .iter()
        .find(|workspace| workspace.id == plain.id)
        .expect("the plain folder's workspace");
    assert_eq!(listed.project_id, plain.project_id);
    assert_eq!(listed.path, plain.path);
    assert_eq!(listed.kind, "folder");
    assert_eq!(listed.state, "ready");
    assert_eq!(listed.branch, None);
    assert!(!listed.missing);
    assert!(all.iter().any(|workspace| workspace.id == other.id));

    let only = within(&test, host.workspaces(Some(plain.project_id.clone()))).expect("filtered");
    let ids: Vec<&str> = only.iter().map(|workspace| workspace.id.as_str()).collect();
    assert_eq!(ids, [plain.id.as_str()]);
    test.block_on(stop(running));
}

#[test]
fn git_status_reads_the_branch_and_the_changed_files_and_refuses_a_folder() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().unwrap();
    let running = test.block_on(start(&home, 0));
    let workspace = git_workspace(&test, &home);
    std::fs::write(Path::new(&workspace.path).join("new.txt"), "draft\n")
        .expect("an untracked file");
    let (_massorca, host) = connected(&test, &home);

    let status = within(&test, host.git_status(workspace.id.clone())).expect("the status");
    assert_eq!(status.branch, workspace.branch);
    assert!(status.branch.is_some(), "{status:?}");
    assert!(!status.truncated);
    let file = status
        .files
        .iter()
        .find(|file| file.path == "new.txt")
        .expect("the new file");
    assert!(file.untracked);
    assert_eq!(file.staged, None);
    assert_eq!(file.unstaged, None);

    let folder = folder_workspace(&test, &home, "plain");
    let refusal = within(&test, host.git_status(folder.id)).expect_err("a folder has no git");
    assert_eq!(refusal.code(), codes::GIT_NOT_A_REPOSITORY, "{refusal:?}");
    test.block_on(stop(running));
}

#[test]
fn watch_git_delivers_its_first_item_and_ends_when_closed() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().unwrap();
    let running = test.block_on(start(&home, 0));
    let workspace = git_workspace(&test, &home);
    let (_massorca, host) = connected(&test, &home);

    let watch = host.watch_git(workspace.id.clone());
    let first = within(&test, watch.next()).expect("the first item");
    let item = match first {
        Some(GitEvent::Item { item }) => item,
        other => panic!("expected the first item, got {other:?}"),
    };
    assert!(item.ready, "{item:?}");
    assert!(item.kinds.is_empty(), "{item:?}");

    watch.close();
    assert_eq!(within(&test, watch.next()).expect("closed"), None);
    test.block_on(stop(running));
}

#[test]
fn a_refused_watch_reports_the_refusal_once_and_then_ends() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().unwrap();
    let running = test.block_on(start(&home, 0));
    let folder = folder_workspace(&test, &home, "plain");
    let (_massorca, host) = connected(&test, &home);

    let watch = host.watch_git(folder.id);
    let refusal = within(&test, watch.next()).expect_err("a folder has no git state");
    assert_eq!(refusal.code(), codes::GIT_NOT_A_REPOSITORY, "{refusal:?}");
    assert_eq!(
        within(&test, watch.next()).expect("the watch is over"),
        None
    );
    test.block_on(stop(running));
}

/// A loopback port nothing listens on now, so a service can start on it, stop and start again.
fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .expect("a free port")
        .local_addr()
        .expect("the port's address")
        .port()
}

#[test]
fn watch_agents_reports_the_snapshot_then_unverifiable_then_live_after_a_restart() {
    let (_temp, home) = temp_home();
    let port = free_port();
    let test = Runtime::new().unwrap();
    let running = test.block_on(start(&home, port));
    let (_massorca, host) = connected(&test, &home);

    let watch = host.watch_agents(None);
    let first = within(&test, watch.next()).expect("the first item");
    assert!(
        matches!(
            first,
            Some(AgentEvent::Item {
                item: AgentItem::Snapshot { .. }
            })
        ),
        "{first:?}"
    );

    test.block_on(stop(running));
    let lost = within(&test, watch.next()).expect("the loss is reported");
    assert!(
        matches!(
            lost,
            Some(AgentEvent::Link {
                link: LinkState::Unverifiable { attempt: 1, .. }
            })
        ),
        "{lost:?}"
    );

    let running = test.block_on(start(&home, port));
    let mut attempts = Vec::new();
    loop {
        match within(&test, watch.next()).expect("the watch goes on") {
            Some(AgentEvent::Link {
                link: LinkState::Unverifiable { attempt, .. },
            }) => attempts.push(attempt),
            Some(AgentEvent::Link {
                link: LinkState::Live,
            }) => break,
            other => panic!("expected the host to return, got {other:?}"),
        }
        assert!(
            attempts.len() < 60,
            "the host never came back: {attempts:?}"
        );
    }
    assert!(
        attempts.windows(2).all(|pair| pair[0] < pair[1]),
        "{attempts:?}"
    );
    let again = within(&test, watch.next()).expect("the snapshot after the return");
    assert!(
        matches!(
            again,
            Some(AgentEvent::Item {
                item: AgentItem::Snapshot { .. }
            })
        ),
        "{again:?}"
    );

    watch.close();
    assert_eq!(within(&test, watch.next()).expect("closed"), None);
    test.block_on(stop(running));
}

#[test]
fn close_ends_a_next_that_is_waiting_for_an_item() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().unwrap();
    let running = test.block_on(start(&home, 0));
    let (_massorca, host) = connected(&test, &home);
    let watch = host.watch_agents(None);
    within(&test, watch.next()).expect("the snapshot");

    let waiting = test.block_on(async {
        let (ended, ()) = tokio::join!(timeout(WAIT, watch.next()), async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            watch.close();
        });
        ended.expect("ended in time")
    });
    assert_eq!(waiting.expect("no refusal"), None);
    test.block_on(stop(running));
}
