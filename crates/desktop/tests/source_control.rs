//! The Source control panel's calls against a real service in a temporary profile, driven headlessly:
//! status, the diff of a file, stage, commit and history, the watch that refetches when a file changes
//! outside the desktop, and a discard that the service refuses until its waiver is sent. Git runs in a
//! temporary repository; no window opens and no user repository is touched.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::{Client, ConnectOptions};
use massorca_desktop::service::{
    ConnectionState, GitUpdate, Refusal, ServiceBridge, ServiceHandle, StoreState, StoreUpdate,
    connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::methods::git::commit::GitCommitParams;
use massorca_protocol::methods::git::diff::GitDiffFileParams;
use massorca_protocol::methods::git::index::{GitDiscardParams, GitPathsParams};
use massorca_protocol::methods::git::remote::{
    GitFetchParams, GitPullParams, GitPulled, GitPushParams, PullMode, PushOutcome,
};
use massorca_protocol::methods::git::status::GitStatus;
use massorca_protocol::methods::git::{ChangeKind, LineCounts};
use massorca_protocol::methods::projects::{
    ProjectAddParams, ProjectRemoveParams, ProjectSetBaseParams,
};
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};
use massorca_protocol::{PeerInfo, codes};
use massorca_server::{RunningService, Service, ServiceConfig};
use tempfile::TempDir;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(30);

/// A private home below a temporary folder; the temporary folder itself may be shared with other users (ADR 0073).
struct TempHome {
    _temp: TempDir,
    path: PathBuf,
}

impl TempHome {
    fn path(&self) -> &Path {
        &self.path
    }
}

fn temp_home() -> TempHome {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let path = temp.path().join("home");
    massorca_profile::create_home(&path).expect("a private home");
    TempHome { _temp: temp, path }
}

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
    // The app's own commits read this identity; CI runners have none.
    git(dir, &["config", "user.name", "MassOrca Test"]);
    git(dir, &["config", "user.email", "test@example.com"]);
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
        ConnectOptions::new(PeerInfo { ..base_peer_info() }),
    )
    .await
    .expect("a client connects");
    let project = client
        .project_add(&ProjectAddParams {
            path: repo_dir.display().to_string(),
            name: Some("demo".to_owned()),
            workspaces_root: Some(
                repo_dir
                    .parent()
                    .expect("a parent")
                    .join("demo-worktrees")
                    .display()
                    .to_string(),
            ),
            ..base_project_add_params()
        })
        .await
        .expect("the project is added");
    let workspace = client
        .workspace_create(&WorkspaceCreateParams {
            in_place: false,
            project: project.id.clone(),
            name: "feature".to_owned(),
            ..base_workspace_create_params()
        })
        .await
        .expect("the workspace is created");
    (project.id, workspace)
}

/// The updates a bounded wait folded, so a wait that runs out can say what it saw.
#[derive(Default)]
struct Trace {
    seen: Vec<String>,
    /// Watch items folded so far: the ready item, then one per change the watch reported.
    watch_items: usize,
}

impl Trace {
    fn fold(&mut self, state: &mut StoreState, update: StoreUpdate) {
        if matches!(update, StoreUpdate::Git(GitUpdate::Watch { .. })) {
            self.watch_items += 1;
        }
        let line: String = format!("{update:?}").chars().take(160).collect();
        self.seen.push(line);
        state.apply(update, Instant::now());
    }

    fn report(&self) -> String {
        let tail: Vec<&str> = self
            .seen
            .iter()
            .rev()
            .take(12)
            .rev()
            .map(String::as_str)
            .collect();
        format!(
            "updates folded: {}, watch items: {}\nlast updates:\n  {}",
            self.seen.len(),
            self.watch_items,
            tail.join("\n  ")
        )
    }
}

/// The store as the panel reads it for `workspace`, in one line.
fn describe(state: &StoreState, workspace: &str) -> String {
    let git = state.git(workspace);
    let status = match git.and_then(|git| git.status.as_ref()) {
        None => "no status yet".to_owned(),
        Some(Err(refusal)) => format!("status refused: {}", refusal.message()),
        Some(Ok(status)) => {
            let paths: Vec<String> = status
                .entries
                .iter()
                .map(|entry| {
                    let untracked = if entry.untracked { " (untracked)" } else { "" };
                    format!("{}{untracked}", entry.path)
                })
                .collect();
            format!("status entries {paths:?}")
        }
    };
    let degraded = git.and_then(|git| git.degraded.clone());
    format!(
        "connected: {}; {status}; degraded: {degraded:?}",
        state.connection.is_connected()
    )
}

/// Folds the bridge's updates until `done` holds. A wait that runs out fails with the time it took, the
/// last store state and the updates it folded, never with a bare timeout.
async fn fold_until(
    updates: &mut UnboundedReceiver<StoreUpdate>,
    state: &mut StoreState,
    trace: &mut Trace,
    workspace: &str,
    what: &str,
    done: impl Fn(&StoreState, &Trace) -> bool,
) {
    let started = Instant::now();
    while !done(state, trace) {
        let remaining = WAIT.saturating_sub(started.elapsed());
        let context = || {
            format!(
                "after {:.1?} waiting for {what}\nlast store: {}\n{}",
                started.elapsed(),
                describe(state, workspace),
                trace.report()
            )
        };
        match timeout(remaining, updates.recv()).await {
            Ok(Some(update)) => trace.fold(state, update),
            Ok(None) => panic!("the bridge closed {}", context()),
            Err(_) => panic!("timed out {}", context()),
        }
    }
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
    trace: Trace,
    workspace: Workspace,
}

impl Session {
    async fn start(home: &TempHome) -> (Self, PathBuf) {
        let repo_dir = home.path().join("demo");
        repo(&repo_dir);
        let service = start_service(home.path()).await;
        let (project_id, workspace) = project_and_workspace(home.path(), &repo_dir).await;
        let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
            .expect("the bridge starts");
        let handle = bridge.handle();
        let mut updates = bridge.take_updates().expect("the updates are there");
        let mut state = StoreState::default();
        let mut trace = Trace::default();
        let listed = workspace.id.clone();
        fold_until(
            &mut updates,
            &mut state,
            &mut trace,
            &listed,
            "the workspace",
            |s, _| s.connection.is_connected() && s.loaded && s.workspace(&listed).is_some(),
        )
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
                trace,
                workspace,
            },
            worktree,
        )
    }

    fn id(&self) -> &str {
        &self.workspace.id
    }

    /// Folds the bridge's updates until `done` holds.
    async fn until(&mut self, what: &str, done: impl Fn(&StoreState) -> bool) {
        fold_until(
            &mut self.updates,
            &mut self.state,
            &mut self.trace,
            &self.workspace.id,
            what,
            |state, _| done(state),
        )
        .await;
    }

    /// Folds the bridge's updates until the watch has delivered `items` items in all, the first being its
    /// ready item.
    async fn until_watch(&mut self, what: &str, items: usize) {
        fold_until(
            &mut self.updates,
            &mut self.state,
            &mut self.trace,
            &self.workspace.id,
            what,
            |_, trace| trace.watch_items >= items,
        )
        .await;
    }

    /// Folds the bridge's updates for `window` and returns how many watch items arrived in it.
    async fn during(&mut self, window: Duration) -> usize {
        let before = self.trace.watch_items;
        let (updates, state, trace) = (&mut self.updates, &mut self.state, &mut self.trace);
        let _ = timeout(window, async {
            loop {
                let update = updates.recv().await.expect("the bridge is alive");
                trace.fold(state, update);
            }
        })
        .await;
        self.trace.watch_items - before
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
    let home = temp_home();
    let (mut session, worktree) = Session::start(&home).await;
    session.handle.git_watch(session.id());
    std::fs::write(worktree.join("README.md"), "hello\nworld\n").expect("an edit");

    // The watch reads the status, which lists the edit as an unstaged change with its line counts.
    let id = session.id().to_owned();
    session
        .until("the edit", |s| {
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
            ..base_line_counts()
        })
    );
    session
        .until("the history", |s| {
            s.git(&id)
                .is_some_and(|g| matches!(&g.log, Some(Ok(log)) if !log.commits.is_empty()))
        })
        .await;

    // The diff of the file, with the patch text the view draws.
    let diff = session
        .handle
        .git_diff_file(GitDiffFileParams {
            workspace: id.clone(),
            ..base_git_diff_file_params()
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
            ..base_git_paths_params()
        })
        .await
        .expect("the file is staged");
    session
        .until("the staged file", |s| {
            status(s, &id).is_some_and(|st| {
                st.entries
                    .iter()
                    .any(|e| e.path == "README.md" && e.staged == Some(ChangeKind::Modified))
            })
        })
        .await;

    // With nothing staged, a commit is refused under its own code.
    session
        .handle
        .git_commit(GitCommitParams {
            workspace: id.clone(),
            message: Some("update the readme".into()),
            ..base_git_commit_params()
        })
        .await
        .expect("the staged file is committed");
    session.until("the new commit", |s| {
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
            ..base_git_commit_params()
        })
        .await
        .expect_err("nothing is staged");
    assert_eq!(refused.code(), Some(codes::GIT_NOTHING_TO_COMMIT));

    session.stop().await;
}

#[tokio::test]
async fn an_outside_change_reaches_the_store_and_a_discard_waits_for_its_waiver() {
    let home = temp_home();
    let (mut session, worktree) = Session::start(&home).await;
    session.handle.git_watch(session.id());
    // The watch's ready item comes first and is sent once its OS watcher is armed (core #117), so an edit
    // made after it cannot fall between the subscription and the watcher.
    session.until_watch("the watch to be ready", 1).await;
    let id = session.id().to_owned();
    session
        .until("the first status", |s| status(s, &id).is_some())
        .await;

    // A file written outside the desktop (by an editor or an agent) arrives through the watch.
    std::fs::write(worktree.join("notes.txt"), "draft\n").expect("an untracked file");
    session
        .until("the untracked file", |s| {
            status(s, &id).is_some_and(|st| {
                st.entries
                    .iter()
                    .any(|e| e.path == "notes.txt" && e.untracked)
            })
        })
        .await;

    // Discarding it is refused, and the refusal names the file, until the waiver is sent.
    let refused = session
        .handle
        .git_discard(GitDiscardParams {
            workspace: id.clone(),
            paths: vec!["notes.txt".into()],
            ..base_git_discard_params()
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
            delete_untracked: true,
            ..base_git_discard_params()
        })
        .await
        .expect("the waiver deletes the file");
    assert_eq!(discarded.deleted, vec!["notes.txt".to_owned()]);
    assert!(!worktree.join("notes.txt").exists());
    session
        .until("the clean status", |s| {
            status(s, &id).is_some_and(|st| st.entries.is_empty())
        })
        .await;

    session.stop().await;
}

#[tokio::test]
async fn a_reconnect_re_establishes_the_watch_with_one_subscription_per_workspace() {
    let home = temp_home();
    let (mut session, worktree) = Session::start(&home).await;
    let id = session.id().to_owned();
    session.handle.git_watch(&id);
    session
        .until("the first status", |s| status(s, &id).is_some())
        .await;

    // The link drops and comes back. The watch the panel asked for starts again on the new connection.
    let service = session.service.take().expect("the service runs");
    stop_service(service).await;
    session
        .until("the lost link", |s| {
            matches!(s.connection, ConnectionState::Reconnecting { .. })
        })
        .await;
    session.service = Some(start_service(home.path()).await);

    // One subscription answers after the reconnection: its first item, and nothing from a second one.
    let from = session.trace.watch_items;
    session
        .until_watch("the watch after the reconnection", from + 1)
        .await;
    let ready = session.trace.watch_items - from;
    assert_eq!(ready, 1, "one subscription sends its first item");
    let quiet = session.during(Duration::from_millis(600)).await;
    assert_eq!(quiet, 0, "no second subscription sends a first item");

    // A change made outside the desktop reaches the store, and one subscription reports it once.
    std::fs::write(worktree.join("notes.txt"), "draft\n").expect("an untracked file");
    let from = session.trace.watch_items;
    session
        .until("the change after the reconnection", |s| {
            status(s, &id).is_some_and(|st| {
                st.entries
                    .iter()
                    .any(|e| e.path == "notes.txt" && e.untracked)
            })
        })
        .await;
    let seen_at_once = session.trace.watch_items - from;
    let after = session.during(Duration::from_millis(600)).await;
    assert_eq!(
        seen_at_once + after,
        1,
        "one subscription reports the change once"
    );

    session.stop().await;
}

/// Every `WorkspaceCreateParams` field at its usual value; tests override the fields they vary.
fn base_workspace_create_params() -> massorca_protocol::methods::workspaces::WorkspaceCreateParams {
    massorca_protocol::methods::workspaces::WorkspaceCreateParams {
        project: String::new(),
        name: "main".to_owned(),
        branch: None,
        base: None,
        existing_branch: false,
        refresh_base: false,
        path: None,
        request_id: None,
        no_setup: false,
        unique_name: false,
        name_generated: false,
        in_place: false,
    }
}

/// Every `PeerInfo` field at its usual value; tests override the fields they vary.
fn base_peer_info() -> massorca_protocol::PeerInfo {
    massorca_protocol::PeerInfo {
        name: "massorca-test-client".to_owned(),
        version: "0.0.0-test".to_owned(),
    }
}

/// Every `ProjectAddParams` field at its usual value; tests override the fields they vary.
fn base_project_add_params() -> massorca_protocol::methods::projects::ProjectAddParams {
    massorca_protocol::methods::projects::ProjectAddParams {
        path: String::new(),
        name: None,
        kind: None,
        workspaces_root: None,
        icon: None,
    }
}

/// Every `GitCommitParams` field at its usual value; tests override the fields they vary.
fn base_git_commit_params() -> massorca_protocol::methods::git::commit::GitCommitParams {
    massorca_protocol::methods::git::commit::GitCommitParams {
        workspace: String::new(),
        message: None,
        amend: false,
        allow_published: false,
    }
}

/// Every `GitDiffFileParams` field at its usual value; tests override the fields they vary.
fn base_git_diff_file_params() -> massorca_protocol::methods::git::diff::GitDiffFileParams {
    massorca_protocol::methods::git::diff::GitDiffFileParams {
        workspace: String::new(),
        target: massorca_protocol::methods::git::diff::DiffTarget::Worktree,
        path: "README.md".into(),
        original_path: None,
        max_bytes: None,
    }
}

/// Every `GitDiscardParams` field at its usual value; tests override the fields they vary.
fn base_git_discard_params() -> massorca_protocol::methods::git::index::GitDiscardParams {
    massorca_protocol::methods::git::index::GitDiscardParams {
        workspace: String::new(),
        paths: Vec::new(),
        all: false,
        delete_untracked: false,
        include_staged: false,
        dry_run: false,
    }
}

/// Every `GitPathsParams` field at its usual value; tests override the fields they vary.
fn base_git_paths_params() -> massorca_protocol::methods::git::index::GitPathsParams {
    massorca_protocol::methods::git::index::GitPathsParams {
        workspace: String::new(),
        paths: Vec::new(),
        all: false,
    }
}

/// Every `LineCounts` field at its usual value; tests override the fields they vary.
fn base_line_counts() -> massorca_protocol::methods::git::LineCounts {
    massorca_protocol::methods::git::LineCounts {
        added: 1,
        removed: 0,
        binary: false,
    }
}

fn push_params(workspace: &str) -> GitPushParams {
    GitPushParams {
        workspace: workspace.to_owned(),
        remote: None,
        force_with_lease: None,
        timeout_ms: None,
    }
}

fn fetch_params(workspace: &str) -> GitFetchParams {
    GitFetchParams {
        workspace: workspace.to_owned(),
        remote: None,
        all: false,
        timeout_ms: None,
    }
}

fn pull_params(workspace: &str) -> GitPullParams {
    GitPullParams {
        workspace: workspace.to_owned(),
        mode: PullMode::Merge,
        timeout_ms: None,
    }
}

/// Fetch, pull and push reach a local remote through the service. The push creates the branch on origin
/// and sets it as the upstream; a commit pushed from another clone is fetched, then pulled in.
#[tokio::test]
async fn fetch_pull_and_push_reach_a_local_remote_through_the_service() {
    let home = temp_home();
    let (mut session, worktree) = Session::start(&home).await;
    let id = session.id().to_owned();
    session.handle.git_watch(&id);
    session
        .until("the status", |s| status(s, &id).is_some())
        .await;
    let branch = status(&session.state, &id)
        .and_then(|st| st.branch.head)
        .expect("the worktree is on a branch");

    let remote = home.path().join("remote.git");
    std::fs::create_dir_all(&remote).expect("the remote folder");
    git(&remote, &["init", "--bare", "-q"]);
    let remote_url = remote.display().to_string();
    git(&worktree, &["remote", "add", "origin", &remote_url]);

    let pushed = session
        .handle
        .git_push(push_params(&id))
        .await
        .expect("the push succeeds");
    assert_eq!(pushed.outcome, PushOutcome::Created);
    assert!(pushed.upstream_set, "the first push sets the upstream");

    let other = home.path().join("other");
    let other_url = other.display().to_string();
    git(
        home.path(),
        &["clone", "-q", "--branch", &branch, &remote_url, &other_url],
    );
    std::fs::write(other.join("remote.txt"), "from another clone\n").expect("a file");
    git(&other, &["add", "remote.txt"]);
    git(&other, &["commit", "-q", "-m", "elsewhere"]);
    git(&other, &["push", "-q", "origin", &branch]);

    let fetched = session
        .handle
        .git_fetch(fetch_params(&id))
        .await
        .expect("the fetch succeeds");
    assert!(
        !fetched.updated.is_empty(),
        "the fetch reports the branch that moved"
    );

    let pulled = session
        .handle
        .git_pull(pull_params(&id))
        .await
        .expect("the pull succeeds");
    assert!(
        matches!(pulled, GitPulled::FastForwarded { .. }),
        "the worktree was only behind: {pulled:?}"
    );
    assert!(
        worktree.join("remote.txt").exists(),
        "the pulled file is in the worktree"
    );
    session.stop().await;
}

fn remove_params(project: &str, forget_workspaces: bool) -> ProjectRemoveParams {
    ProjectRemoveParams {
        project: project.to_owned(),
        forget_workspaces,
    }
}

/// A project with workspace records is refused until its waiver forgets them; the worktree stays on disk.
#[tokio::test]
async fn removing_a_project_with_workspaces_waits_for_the_forget_waiver() {
    let home = temp_home();
    let (session, worktree) = Session::start(&home).await;
    let project = session.workspace.project_id.clone();

    let refused = session
        .handle
        .remove_project(remove_params(&project, false))
        .await;
    let Err(refusal) = refused else {
        panic!("a project with workspaces is refused");
    };
    assert_eq!(refusal.code(), Some(codes::PROJECT_HAS_WORKSPACES));

    let removed = session
        .handle
        .remove_project(remove_params(&project, true))
        .await
        .expect("the waiver removes the project");
    assert_eq!(removed.id, project);
    assert!(worktree.exists(), "the worktree stays on disk");
    session.stop().await;
}

fn set_base_params(project: &str, base: Option<String>) -> ProjectSetBaseParams {
    ProjectSetBaseParams {
        project: project.to_owned(),
        base,
    }
}

/// The base branch is set to a branch the repository has, then cleared, both through the service.
#[tokio::test]
async fn the_base_branch_is_set_and_cleared_through_the_service() {
    let home = temp_home();
    let (mut session, _worktree) = Session::start(&home).await;
    let id = session.id().to_owned();
    session.handle.git_watch(&id);
    session
        .until("the status", |s| status(s, &id).is_some())
        .await;
    let branch = status(&session.state, &id)
        .and_then(|st| st.branch.head)
        .expect("the worktree is on a branch");
    let project = session.workspace.project_id.clone();

    let set = session
        .handle
        .set_project_base(set_base_params(&project, Some(branch.clone())))
        .await
        .expect("the base is set");
    assert_eq!(set.base_ref, Some(branch));

    let cleared = session
        .handle
        .set_project_base(set_base_params(&project, None))
        .await
        .expect("the base is cleared");
    assert_eq!(cleared.base_ref, None);
    session.stop().await;
}

#[tokio::test]
async fn a_moved_submodule_pointer_reaches_the_diff_with_both_commits() {
    let home = temp_home();
    let (session, worktree) = Session::start(&home).await;
    let sub = home.path().with_file_name("sub");
    repo(&sub);
    let sub_path = sub.to_str().expect("a UTF-8 path");
    git(
        &worktree,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            sub_path,
            "sub",
        ],
    );
    git(&worktree, &["commit", "-q", "-m", "add the submodule"]);
    git(
        &worktree.join("sub"),
        &["commit", "-q", "--allow-empty", "-m", "move the pointer"],
    );

    let diff = session
        .handle
        .git_diff_file(GitDiffFileParams {
            workspace: session.id().to_owned(),
            path: "sub".into(),
            ..base_git_diff_file_params()
        })
        .await
        .expect("the pointer diff is read");
    let pointer = diff
        .submodule
        .expect("the pointer is named, not left as an empty diff");
    assert!(diff.patch.is_none(), "a pointer carries no patch text");
    assert_ne!(pointer.old_commit, pointer.new_commit, "the pointer moved");
    assert_eq!(
        pointer.dirty,
        Some(false),
        "the submodule's checkout is clean"
    );
    assert!(
        pointer.summary.starts_with("Subproject commit "),
        "{}",
        pointer.summary
    );
    session.stop().await;
}
