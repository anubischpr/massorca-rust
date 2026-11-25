//! The Sidebar's session rows on a real service (A97). A project with two sessions and a delegated child
//! produces session rows under its host and no workspace row between them. The store is filled from the
//! bridge's own updates, as the app fills it, and the tree is read from the snapshot the Sidebar draws from.
//! Throwaway profile only; the fake agent is the platform shell, as in agent_bridge.rs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_desktop::model::navigation::Mode;
use massorca_desktop::model::service_view::service_snapshot;
use massorca_desktop::model::shell::HostReach;
use massorca_desktop::model::sidebar::{SidebarContent, TreeRow, sidebar_content};
use massorca_desktop::service::{ServiceBridge, ServiceHandle, StoreState, connect_options};
use massorca_profile::Profile;
use massorca_protocol::methods::agents::{AgentDelegateParams, AgentSession, AgentStartParams};
use massorca_protocol::methods::projects::ProjectAddParams;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};
use massorca_server::{RunningService, Service, ServiceConfig};
use tempfile::TempDir;
use tokio::time::timeout;

/// A private home below a temporary folder, as agent_bridge.rs makes it.
struct TempHome {
    _temp: TempDir,
    path: PathBuf,
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

/// Declares kind `fake`, which runs the platform shell (see agent_bridge.rs).
fn declare_fake_kind(home: &Path) {
    let dir = profile(home).dir().join("agents").join("kinds");
    std::fs::create_dir_all(&dir).expect("the kinds folder");
    let program = if cfg!(windows) { "cmd.exe" } else { "/bin/sh" };
    let kind = format!(r#"{{"id":"fake","name":"Fake agent","program":["{program}"]}}"#);
    std::fs::write(dir.join("fake.json"), kind).expect("the kind file");
}

async fn add_workspace(handle: &ServiceHandle, repo_dir: &Path) -> Workspace {
    let project = handle
        .add_project(ProjectAddParams {
            icon: None,
            path: repo_dir.display().to_string(),
            name: Some("demo".to_owned()),
            kind: None,
            workspaces_root: repo_dir
                .parent()
                .map(|parent| parent.join("demo-worktrees").display().to_string()),
        })
        .await
        .expect("the project is added");
    handle
        .create_workspace(WorkspaceCreateParams {
            project: project.id,
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
        })
        .await
        .expect("the workspace is created")
}

async fn start_fake(handle: &ServiceHandle, workspace: &str) -> AgentSession {
    handle
        .agent_start(AgentStartParams {
            workspace: Some(workspace.to_owned()),
            kind: "fake".to_owned(),
            prompt: None,
            args: Vec::new(),
            env: BTreeMap::new(),
            read_only: false,
            title: None,
            cols: Some(100),
            rows: Some(30),
            request_id: None,
            permission_mode: None,
            model: None,
            effort: None,
        })
        .await
        .expect("the session starts")
}

fn tree_of(state: &StoreState) -> Vec<TreeRow> {
    let snapshot = service_snapshot(state, Instant::now());
    let SidebarContent::Workspaces { tree, .. } = sidebar_content(&snapshot, Mode::Workspaces)
    else {
        panic!("workspaces content expected");
    };
    tree
}

fn session_rows(tree: &[TreeRow]) -> usize {
    tree.iter()
        .filter(|row| matches!(row, TreeRow::Session { .. }))
        .count()
}

#[tokio::test]
async fn sessions_and_a_delegated_child_are_rows_under_their_host_with_no_workspace_row() {
    let home = temp_home();
    let repo_dir = home.path.with_file_name("repo");
    repo(&repo_dir);
    declare_fake_kind(&home.path);
    let service = start_service(&home.path).await;
    let mut bridge = ServiceBridge::start(profile(&home.path), connect_options(None))
        .expect("the bridge starts");
    let handle = bridge.handle();
    let mut updates = bridge.take_updates().expect("the updates are there");
    // The handle answers only once the link is up; the updates wait in the channel for the store.
    timeout(WAIT, async {
        while !handle.connection().is_connected() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the service connects");

    let workspace = add_workspace(&handle, &repo_dir).await;
    let first = start_fake(&handle, &workspace.id).await;
    // The service lets one session write in a workspace at a time, so the second session gets its own.
    let other = handle
        .create_workspace(WorkspaceCreateParams {
            project: workspace.project_id.clone(),
            name: "second".to_owned(),
            branch: None,
            base: None,
            existing_branch: false,
            refresh_base: false,
            path: None,
            request_id: None,
            no_setup: false,
            unique_name: false,
            name_generated: false,
        })
        .await
        .expect("the second workspace is created");
    let second = start_fake(&handle, &other.id).await;
    handle
        .agent_delegate(AgentDelegateParams {
            parent: first.id.clone(),
            prompt: "look".to_owned(),
            kind: Some("fake".to_owned()),
            name: None,
            title: Some("Check".to_owned()),
            read_only: true,
            token: None,
            request_id: None,
            permission_mode: None,
        })
        .await
        .expect("the child starts under its parent");

    // Fill the store the way the app does: every bridge update goes into it, until the three sessions show.
    let mut state = StoreState::default();
    timeout(WAIT, async {
        loop {
            let update = updates.recv().await.expect("the bridge is alive");
            state.apply(update, Instant::now());
            if session_rows(&tree_of(&state)) == 3 {
                return;
            }
        }
    })
    .await
    .expect("the sessions reach the store");

    let tree = tree_of(&state);
    assert_eq!(tree.len(), 5, "project, host, then the three session rows");
    assert!(matches!(
        &tree[0],
        TreeRow::Project { name, .. } if name == "demo"
    ));
    assert!(matches!(
        &tree[1],
        TreeRow::Host { name, reach: HostReach::Connected } if name == "This PC"
    ));

    let child = state
        .agents
        .iter()
        .find(|session| session.parent_id.as_deref() == Some(first.id.as_str()))
        .expect("the child is in the store")
        .id
        .clone();
    let branch = state
        .workspace(&workspace.id)
        .map(|workspace| {
            workspace
                .branch
                .clone()
                .unwrap_or_else(|| workspace.name.clone())
        })
        .expect("the workspace is in the store");
    let rows: Vec<(String, usize)> = tree[2..]
        .iter()
        .filter_map(|row| match row {
            TreeRow::Session { id, depth, .. } => Some((id.clone(), *depth)),
            _ => None,
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            (first.id.clone(), 0),
            (child.clone(), 1),
            (second.id.clone(), 0)
        ],
        "oldest first, the child under its parent"
    );
    let titles: Vec<String> = tree[2..]
        .iter()
        .filter_map(|row| match row {
            TreeRow::Session { title, .. } => Some(title.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(titles[0], branch, "an untitled session shows its branch");
    let other_branch = state
        .workspace(&other.id)
        .map(|w| w.branch.clone().unwrap_or_else(|| w.name.clone()))
        .expect("the second workspace is in the store");
    assert_eq!(
        titles[2], other_branch,
        "an untitled session shows its branch"
    );
    assert_eq!(titles[1], "Check", "the child keeps the title it was given");

    drop(bridge);
    stop_service(service).await;
}
