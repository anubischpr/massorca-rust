//! The bridge against a real service in a temporary profile, driven headlessly: no window opens. The
//! agent sessions and the agent kinds the service lists reach the store on connect, the way workspaces and
//! terminals do.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::Client;
use massorca_desktop::model::agent_sessions::bring_back_refusal;
use massorca_desktop::service::{ServiceBridge, StoreState, StoreUpdate, connect_options};
use massorca_profile::Profile;
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{AgentBringBackParams, AgentStartParams, PackageState};
use massorca_protocol::methods::projects::ProjectAddParams;
use massorca_protocol::methods::workspaces::WorkspaceCreateParams;
use massorca_server::{RunningService, Service, ServiceConfig};
use tempfile::TempDir;
use tokio::time::timeout;

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

#[tokio::test]
async fn the_agent_sessions_the_service_lists_reach_the_store_on_connect() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let mut updates = bridge.take_updates().expect("the updates are there");

    let listed = timeout(WAIT, async {
        loop {
            let update = updates.recv().await.expect("the bridge is alive");
            if let StoreUpdate::AgentsResync(sessions) = update {
                return sessions;
            }
        }
    })
    .await
    .expect("the agent sessions arrive after the connect");
    assert!(
        listed.is_empty(),
        "a new profile has no agent sessions: {listed:?}"
    );
    // An empty list also comes from a service that predates agents, so ask the service directly too.
    let client = Client::connect(&profile(home.path()), connect_options(None))
        .await
        .expect("a client connects");
    let direct = client
        .agent_list(None)
        .await
        .expect("the service answers agent.list");
    assert!(direct.sessions.is_empty());

    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn the_agent_kinds_the_service_declares_reach_the_store_on_connect() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let mut updates = bridge.take_updates().expect("the updates are there");

    let declared = timeout(WAIT, async {
        loop {
            let update = updates.recv().await.expect("the bridge is alive");
            if let StoreUpdate::AgentKindsResync(kinds) = update {
                return kinds;
            }
        }
    })
    .await
    .expect("the agent kinds arrive after the connect");
    // The four built-in kinds, in the order the service lists them, each with its package enabled.
    let ids: Vec<&str> = declared.iter().map(|kind| kind.id.as_str()).collect();
    assert_eq!(ids, ["claude", "codex", "antigravity", "gentle-shell"]);
    assert!(
        declared
            .iter()
            .all(|kind| kind.package.state == PackageState::Enabled)
    );

    let mut store = StoreState::default();
    store.apply(
        StoreUpdate::AgentKindsResync(declared.clone()),
        Instant::now(),
    );
    assert_eq!(store.agent_kinds, declared);

    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn a_bring_back_the_service_refuses_arrives_with_its_code_and_a_plain_sentence() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let handle = bridge.handle();
    let mut updates = bridge.take_updates().expect("the updates are there");
    // The first agent list comes once the bridge is connected, so the call below reaches the service.
    timeout(WAIT, async {
        loop {
            if let StoreUpdate::AgentsResync(_) = updates.recv().await.expect("the bridge is alive")
            {
                return;
            }
        }
    })
    .await
    .expect("the bridge connects");

    let child = "0196a6f0-0000-7000-8000-000000000000".to_owned();
    let refusal = handle
        .agent_bring_back(AgentBringBackParams {
            child: child.clone(),
            merge: true,
            prompt: true,
            token: None,
        })
        .await
        .expect_err("a session the service does not have is refused");
    assert_eq!(refusal.code(), Some(codes::AGENT_NOT_FOUND), "{refusal:?}");
    assert_eq!(
        bring_back_refusal(&refusal),
        "This session no longer exists."
    );
    // The refetch after the write finds the session gone, whatever the answer was.
    let gone = timeout(WAIT, async {
        loop {
            if let StoreUpdate::AgentGone(id) = updates.recv().await.expect("the bridge is alive") {
                return id;
            }
        }
    })
    .await
    .expect("the refetch after the write reports the session gone");
    assert_eq!(gone, child);

    drop(bridge);
    stop_service(service).await;
}

/// Runs git in `dir` with a fixed identity, and fails when git does.
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

fn repo(dir: &Path) {
    std::fs::create_dir_all(dir).expect("the repository directory");
    git(dir, &["init", "-q"]);
    std::fs::write(dir.join("README.md"), "hello\n").expect("a readme");
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "initial"]);
}

/// Declares kind `fake`, which runs the platform shell: the fake agent is a binary of the CLI crate, which
/// this crate's tests cannot start, and the terminal test uses the same shell.
fn declare_fake_kind(home: &Path) {
    let dir = profile(home).dir().join("agents").join("kinds");
    std::fs::create_dir_all(&dir).expect("the kinds folder");
    let program = if cfg!(windows) { "cmd.exe" } else { "/bin/sh" };
    let kind = format!(r#"{{"id":"fake","name":"Fake agent","program":["{program}"]}}"#);
    std::fs::write(dir.join("fake.json"), kind).expect("the kind file");
}

#[tokio::test]
async fn an_agent_session_starts_in_a_workspace_through_the_bridge_and_reports_its_terminal() {
    let home = temp_home();
    let repo_dir = home.path().with_file_name("repo");
    repo(&repo_dir);
    declare_fake_kind(home.path());
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let handle = bridge.handle();
    let mut updates = bridge.take_updates().expect("the updates are there");
    timeout(WAIT, async {
        loop {
            if let StoreUpdate::AgentKindsResync(kinds) =
                updates.recv().await.expect("the bridge is alive")
                && kinds.iter().any(|kind| kind.id == "fake" && kind.detected)
            {
                return;
            }
        }
    })
    .await
    .expect("the declared kind is listed once the bridge connects");

    let project = handle
        .add_project(ProjectAddParams {
            path: repo_dir.display().to_string(),
            name: Some("demo".to_owned()),
            kind: None,
            workspaces_root: repo_dir
                .parent()
                .map(|parent| parent.join("demo-worktrees").display().to_string()),
        })
        .await
        .expect("the project is added");
    let workspace = handle
        .create_workspace(WorkspaceCreateParams {
            project: project.id,
            name: "main".to_owned(),
            branch: None,
            base: None,
            existing_branch: false,
            path: None,
            request_id: None,
        })
        .await
        .expect("the workspace is created");

    let session = handle
        .agent_start(AgentStartParams {
            workspace: workspace.id.clone(),
            kind: "fake".to_owned(),
            prompt: None,
            args: Vec::new(),
            env: BTreeMap::new(),
            read_only: false,
            title: None,
            cols: Some(100),
            rows: Some(30),
            request_id: None,
        })
        .await
        .expect("the session starts");
    assert_eq!(session.kind, "fake");
    assert_eq!(session.workspace_id, workspace.id);
    let terminal = session
        .terminal_id
        .clone()
        .expect("the session has its terminal");
    // The started session reaches the store as a change, not only through a later refetch.
    let changed = timeout(WAIT, async {
        loop {
            if let StoreUpdate::AgentChanged(changed) =
                updates.recv().await.expect("the bridge is alive")
                && changed.id == session.id
            {
                return changed;
            }
        }
    })
    .await
    .expect("the session reaches the store");
    assert_eq!(changed.terminal_id.as_deref(), Some(terminal.as_str()));

    handle
        .close_terminal(&terminal)
        .await
        .expect("the session's terminal closes");
    drop(bridge);
    stop_service(service).await;
}
