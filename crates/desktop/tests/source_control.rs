//! The Source control panel's calls against a real service in a temporary profile, driven headlessly:
//! status, the diff of a file, stage, commit and history, the watch that refetches when a file changes
//! outside the desktop, and a discard that the service refuses until its waiver is sent. Git runs in a
//! temporary repository; no window opens and no user repository is touched.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use massorca_client::{Client, ConnectOptions};
use massorca_desktop::service::{
    ConnectionState, GitUpdate, Refusal, ServiceBridge, ServiceHandle, StoreState, StoreUpdate,
    connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::methods::git::commit::GitCommitParams;
use massorca_protocol::methods::git::diff::{DiffTarget, GitDiffFileParams};
use massorca_protocol::methods::git::index::{GitDiscardParams, GitPathsParams};
use massorca_protocol::methods::git::status::GitStatus;
use massorca_protocol::methods::git::{ChangeKind, LineCounts};
use massorca_protocol::methods::projects::ProjectAddParams;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};
use massorca_protocol::{PeerInfo, codes};
use massorca_server::{RunningService, Service, ServiceConfig};
use tempfile::TempDir;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(30);

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

async fn start_service(home: &Path) -> RunningService {
    Service::start(ServiceConfig {
        profile: profile(home),
        server_name: "massorca-test".to_owned(),
        server_version: "0.0.0-test".to_owned(),
    })
    .await
    .expect("the service starts")
}

async fn stop_service(running: RunningService) {
    running.shutdown_handle().shutdown();
    timeout(WAIT, running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
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
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A repository with one commit: a readme and a `.gitignore` that ignores `*.log`.
fn repo(dir: &Path) {
    std::fs::create_dir_all(dir).expect("the repository directory");
    git(dir, &["init", "-q"]);
    std::fs::write(dir.join("README.md"), "hello\n").expect("a readme");
    std::fs::write(dir.join(".gitignore"), "*.log\n").expect("a gitignore");
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "initial"]);
}

/// Registers the repository as a project and creates one workspace in it, through a client of the
/// service, as the CLI would. The worktree folder is the workspace's path.
async fn project_and_workspace(home: &Path, repo_dir: &Path) -> (String, Workspace) {
    let client = Client::connect(
        &profile(home),
        ConnectOptions::new(PeerInfo {
            name: "massorca-test-client".to_owned(),
            version: "0.0.0-test".to_owned(),
        }),
    )
    .await
    .expect("a client connects");
    let project = client
        .project_add(&ProjectAddParams {
            path: repo_dir.display().to_string(),
            name: Some("demo".to_owned()),
            kind: None,
            workspaces_root: Some(
                repo_dir
                    .parent()
                    .expect("a parent")
                    .join("demo-worktrees")
                    .display()
                    .to_string(),
            ),
        })
        .await
        .expect("the project is added");
    let workspace = client
        .workspace_create(&WorkspaceCreateParams {
            project: project.id.clone(),
            name: "feature".to_owned(),
            branch: None,
            base: None,
            existing_branch: false,
            path: None,
            request_id: None,
        })
        .await
        .expect("the workspace is created");
    (project.id, workspace)
}

/// Folds the bridge's updates into a store state until `done` holds.
async fn until(
    updates: &mut UnboundedReceiver<StoreUpdate>,
    state: &mut StoreState,
    what: &str,
    done: impl Fn(&StoreState) -> bool,
) {
    let folded = timeout(WAIT, async {
        while !done(state) {
            let update = updates.recv().await.expect("the bridge is alive");
            state.apply(update, std::time::Instant::now());
        }
    })
    .await;
    assert!(folded.is_ok(), "timed out waiting for {what}");
}

/// The workspace's status when the store holds a readable one.
fn status(state: &StoreState, workspace: &str) -> Option<GitStatus> {
    state
        .git(workspace)?
        .status
        .as_ref()?
        .as_ref()
        .ok()
        .cloned()
}

/// A running service, a bridge following it, and the workspace the panel would show.
struct Session {
    service: Option<RunningService>,
    bridge: ServiceBridge,
    handle: ServiceHandle,
    updates: UnboundedReceiver<StoreUpdate>,
    state: StoreState,
    workspace: Workspace,
}

impl Session {
    async fn start(home: &TempDir) -> (Self, PathBuf) {
        let repo_dir = home.path().join("demo");
        repo(&repo_dir);
        let service = start_service(home.path()).await;
        let (project_id, workspace) = project_and_workspace(home.path(), &repo_dir).await;
        let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
            .expect("the bridge starts");
        let handle = bridge.handle();
        let mut updates = bridge.take_updates().expect("the updates are there");
        let mut state = StoreState::default();
        let listed = workspace.id.clone();
        until(&mut updates, &mut state, "the workspace", |s| {
            s.connection.is_connected() && s.loaded && s.workspace(&listed).is_some()
        })
        .await;
        assert!(state.project(&project_id).is_some());
        let worktree = PathBuf::from(&workspace.path);
        (
            Self {
                service: Some(service),
                bridge,
                handle,
                updates,
                state,
                workspace,
            },
            worktree,
        )
    }

    fn id(&self) -> &str {
        &self.workspace.id
    }

    async fn stop(self) {
        let Self {
            service,
            bridge,
            handle,
            ..
        } = self;
        drop(handle);
        drop(bridge);
        if let Some(service) = service {
            stop_service(service).await;
        }
    }
}

#[tokio::test]
async fn status_diff_stage_and_commit_show_in_the_store() {
    let home = tempfile::tempdir().expect("a temporary home");
    let (mut session, worktree) = Session::start(&home).await;
    session.handle.git_watch(session.id());
    std::fs::write(worktree.join("README.md"), "hello\nworld\n").expect("an edit");

    // The watch reads the status, which lists the edit as an unstaged change with its line counts.
    let id = session.id().to_owned();
    until(&mut session.updates, &mut session.state, "the edit", |s| {
        status(s, &id).is_some_and(|st| {
            st.entries
                .iter()
                .any(|e| e.path == "README.md" && e.unstaged.is_some())
        })
    })
    .await;
    let st = status(&session.state, &id).expect("the status is there");
    let readme = st
        .entries
        .iter()
        .find(|e| e.path == "README.md")
        .expect("the readme is listed");
    assert_eq!(readme.unstaged, Some(ChangeKind::Modified));
    assert_eq!(
        readme.unstaged_lines,
        Some(LineCounts {
            added: 1,
            removed: 0,
            binary: false
        })
    );
    until(
        &mut session.updates,
        &mut session.state,
        "the history",
        |s| {
            s.git(&id)
                .is_some_and(|g| matches!(&g.log, Some(Ok(log)) if !log.commits.is_empty()))
        },
    )
    .await;

    // The diff of the file, with the patch text the view draws.
    let diff = session
        .handle
        .git_diff_file(GitDiffFileParams {
            workspace: id.clone(),
            target: DiffTarget::Worktree,
            path: "README.md".into(),
            original_path: None,
            max_bytes: None,
        })
        .await
        .expect("the diff is read");
    assert!(!diff.binary);
    assert!(diff.patch.as_deref().is_some_and(|p| p.contains("+world")));

    // Stage the file: the answer is empty, and the status follows through the refetch.
    session
        .handle
        .git_stage(GitPathsParams {
            workspace: id.clone(),
            paths: vec!["README.md".into()],
            all: false,
        })
        .await
        .expect("the file is staged");
    until(
        &mut session.updates,
        &mut session.state,
        "the staged file",
        |s| {
            status(s, &id).is_some_and(|st| {
                st.entries
                    .iter()
                    .any(|e| e.path == "README.md" && e.staged == Some(ChangeKind::Modified))
            })
        },
    )
    .await;

    // With nothing staged, a commit is refused under its own code.
    session
        .handle
        .git_commit(GitCommitParams {
            workspace: id.clone(),
            message: Some("update the readme".into()),
            amend: false,
            allow_published: false,
        })
        .await
        .expect("the staged file is committed");
    until(&mut session.updates, &mut session.state, "the new commit", |s| {
        s.git(&id).is_some_and(|g| {
            matches!(&g.log, Some(Ok(log)) if log.commits.first().is_some_and(|c| c.subject == "update the readme"))
        }) && status(s, &id).is_some_and(|st| st.entries.is_empty())
    })
    .await;
    let refused = session
        .handle
        .git_commit(GitCommitParams {
            workspace: id.clone(),
            message: Some("nothing here".into()),
            amend: false,
            allow_published: false,
        })
        .await
        .expect_err("nothing is staged");
    assert_eq!(refused.code(), Some(codes::GIT_NOTHING_TO_COMMIT));

    session.stop().await;
}

#[tokio::test]
async fn an_outside_change_reaches_the_store_and_a_discard_waits_for_its_waiver() {
    let home = tempfile::tempdir().expect("a temporary home");
    let (mut session, worktree) = Session::start(&home).await;
    session.handle.git_watch(session.id());
    let id = session.id().to_owned();
    until(
        &mut session.updates,
        &mut session.state,
        "the first status",
        |s| status(s, &id).is_some(),
    )
    .await;

    // A file written outside the desktop (by an editor or an agent) arrives through the watch.
    std::fs::write(worktree.join("notes.txt"), "draft\n").expect("an untracked file");
    until(
        &mut session.updates,
        &mut session.state,
        "the untracked file",
        |s| {
            status(s, &id).is_some_and(|st| {
                st.entries
                    .iter()
                    .any(|e| e.path == "notes.txt" && e.untracked)
            })
        },
    )
    .await;

    // Discarding it is refused, and the refusal names the file, until the waiver is sent.
    let refused = session
        .handle
        .git_discard(GitDiscardParams {
            workspace: id.clone(),
            paths: vec!["notes.txt".into()],
            all: false,
            delete_untracked: false,
            include_staged: false,
            dry_run: false,
        })
        .await
        .expect_err("an untracked file is not deleted unseen");
    match &refused {
        Refusal::GitRefused { code, paths, .. } => {
            assert_eq!(code.as_str(), codes::GIT_DISCARD_UNTRACKED);
            assert_eq!(paths, &vec!["notes.txt".to_owned()]);
        }
        other => panic!("a git refusal was expected, got {other:?}"),
    }
    assert!(
        worktree.join("notes.txt").exists(),
        "a refusal changes nothing"
    );

    let discarded = session
        .handle
        .git_discard(GitDiscardParams {
            workspace: id.clone(),
            paths: vec!["notes.txt".into()],
            all: false,
            delete_untracked: true,
            include_staged: false,
            dry_run: false,
        })
        .await
        .expect("the waiver deletes the file");
    assert_eq!(discarded.deleted, vec!["notes.txt".to_owned()]);
    assert!(!worktree.join("notes.txt").exists());
    until(
        &mut session.updates,
        &mut session.state,
        "the clean status",
        |s| status(s, &id).is_some_and(|st| st.entries.is_empty()),
    )
    .await;

    session.stop().await;
}

/// Folds the bridge's updates into `state` until `done` holds, and counts the watch updates on the way.
async fn until_counting_watch(
    updates: &mut UnboundedReceiver<StoreUpdate>,
    state: &mut StoreState,
    what: &str,
    done: impl Fn(&StoreState, usize) -> bool,
) -> usize {
    async fn fold(
        updates: &mut UnboundedReceiver<StoreUpdate>,
        state: &mut StoreState,
        watch: &mut usize,
        done: &impl Fn(&StoreState, usize) -> bool,
    ) {
        while !done(state, *watch) {
            let update = updates.recv().await.expect("the bridge is alive");
            if matches!(update, StoreUpdate::Git(GitUpdate::Watch { .. })) {
                *watch += 1;
            }
            state.apply(update, std::time::Instant::now());
        }
    }
    let mut watch = 0;
    let folded = timeout(WAIT, fold(updates, state, &mut watch, &done)).await;
    assert!(folded.is_ok(), "timed out waiting for {what}");
    watch
}

/// Folds the bridge's updates into `state` for `window`, and counts the watch updates among them.
async fn watch_updates_during(
    updates: &mut UnboundedReceiver<StoreUpdate>,
    state: &mut StoreState,
    window: Duration,
) -> usize {
    async fn fold(
        updates: &mut UnboundedReceiver<StoreUpdate>,
        state: &mut StoreState,
        watch: &mut usize,
    ) {
        loop {
            let update = updates.recv().await.expect("the bridge is alive");
            if matches!(update, StoreUpdate::Git(GitUpdate::Watch { .. })) {
                *watch += 1;
            }
            state.apply(update, std::time::Instant::now());
        }
    }
    let mut watch = 0;
    let _ = timeout(window, fold(updates, state, &mut watch)).await;
    watch
}

#[tokio::test]
async fn a_reconnect_re_establishes_the_watch_with_one_subscription_per_workspace() {
    let home = tempfile::tempdir().expect("a temporary home");
    let (mut session, worktree) = Session::start(&home).await;
    let id = session.id().to_owned();
    session.handle.git_watch(&id);
    until(
        &mut session.updates,
        &mut session.state,
        "the first status",
        |s| status(s, &id).is_some(),
    )
    .await;

    // The link drops and comes back. The watch the panel asked for starts again on the new connection.
    let service = session.service.take().expect("the service runs");
    stop_service(service).await;
    until(
        &mut session.updates,
        &mut session.state,
        "the lost link",
        |s| matches!(s.connection, ConnectionState::Reconnecting { .. }),
    )
    .await;
    session.service = Some(start_service(home.path()).await);

    // One subscription answers after the reconnection: its first item, and nothing from a second one.
    let ready = until_counting_watch(
        &mut session.updates,
        &mut session.state,
        "the watch after the reconnection",
        |s, watch| s.connection.is_connected() && watch >= 1,
    )
    .await;
    assert_eq!(ready, 1, "one subscription sends its first item");
    let quiet = watch_updates_during(
        &mut session.updates,
        &mut session.state,
        Duration::from_millis(600),
    )
    .await;
    assert_eq!(quiet, 0, "no second subscription sends a first item");

    // A change made outside the desktop reaches the store, and one subscription reports it once.
    std::fs::write(worktree.join("notes.txt"), "draft\n").expect("an untracked file");
    let mut changes = until_counting_watch(
        &mut session.updates,
        &mut session.state,
        "the change after the reconnection",
        |s, _| {
            status(s, &id).is_some_and(|st| {
                st.entries
                    .iter()
                    .any(|e| e.path == "notes.txt" && e.untracked)
            })
        },
    )
    .await;
    changes += watch_updates_during(
        &mut session.updates,
        &mut session.state,
        Duration::from_millis(600),
    )
    .await;
    assert_eq!(changes, 1, "one subscription reports the change once");

    session.stop().await;
}
