//! The bridge against a real service in a temporary profile: a project icon set through the bridge
//! reads back from `project.list`, and a reset clears it (decision 0091).

use std::path::Path;
use std::time::{Duration, Instant};

use massorca_client::{Client, ConnectOptions};
use massorca_desktop::service::{ServiceBridge, ServiceHandle, StoreState, StoreUpdate};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::projects::{ProjectAddParams, ProjectIcon, ProjectUpdateParams};
use massorca_server::{RunningService, Service, ServiceConfig};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(30);

fn peer() -> PeerInfo {
    PeerInfo { ..base_peer_info() }
}

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
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
    assert!(output.status.success(), "git {args:?} failed");
}

fn repo(dir: &Path) {
    std::fs::create_dir_all(dir).expect("the repository directory");
    git(dir, &["init", "-q"]);
    std::fs::write(dir.join("README.md"), "hello\n").expect("a readme");
    git(dir, &["add", "."]);
    git(
        dir,
        &[
            "-c",
            "user.name=massorca-test",
            "-c",
            "user.email=massorca-test@example.invalid",
            "commit",
            "-q",
            "-m",
            "initial",
        ],
    );
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

fn bridge_for(home: &Path) -> (ServiceBridge, ServiceHandle, UnboundedReceiver<StoreUpdate>) {
    let mut bridge = ServiceBridge::start(profile(home), ConnectOptions::new(peer()))
        .expect("the bridge starts");
    let updates = bridge.take_updates().expect("the updates are there");
    let handle = bridge.handle();
    (bridge, handle, updates)
}

/// The icon `project.list` reports for `project`, read through a client of the service's own.
async fn listed_icon(home: &Path, project: &str) -> Option<ProjectIcon> {
    let client = Client::connect(&profile(home), ConnectOptions::new(peer()))
        .await
        .expect("a client connects");
    client
        .project_list()
        .await
        .expect("the projects are listed")
        .projects
        .into_iter()
        .find(|listed| listed.id == project)
        .expect("the project is listed")
        .icon
}

/// Folds the bridge's updates until it has connected to the service.
async fn until_connected(updates: &mut UnboundedReceiver<StoreUpdate>) {
    let mut state = StoreState::default();
    timeout(WAIT, async {
        while !state.connection.is_connected() {
            let update = updates.recv().await.expect("the bridge is alive");
            state.apply(update, Instant::now());
        }
    })
    .await
    .expect("the bridge connects in time");
}

#[tokio::test]
async fn a_project_icon_set_through_the_bridge_reads_back_from_project_list() {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let home = temp.path().join("home");
    massorca_profile::create_home(&home).expect("a private home");
    let repo_dir = temp.path().join("demo");
    repo(&repo_dir);
    let running = start_service(&home).await;
    let (bridge, handle, mut updates) = bridge_for(&home);
    until_connected(&mut updates).await;
    let project = handle
        .add_project(ProjectAddParams {
            path: repo_dir.display().to_string(),
            name: Some("demo".to_owned()),
            ..base_project_add_params()
        })
        .await
        .expect("the project is added")
        .id;
    let star = ProjectIcon::Lucide {
        name: "star".to_owned(),
        color: None,
    };
    handle
        .update_project(ProjectUpdateParams {
            project: project.clone(),
            icon: Some(Some(star.clone())),
        })
        .await
        .expect("the icon is set");
    assert_eq!(listed_icon(&home, &project).await, Some(star));
    handle
        .update_project(ProjectUpdateParams {
            project: project.clone(),
            icon: Some(None),
        })
        .await
        .expect("the icon is reset");
    assert_eq!(listed_icon(&home, &project).await, None);
    drop(bridge);
    stop_service(running).await;
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
