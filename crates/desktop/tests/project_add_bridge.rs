//! The add-project check against a real service in a temporary profile, driven headlessly: no window opens.
//! `project.inspect` says whether a path is a Git repository or a plain folder and adds nothing, and a folder
//! added as a folder stays one (WS-31). The service runs in this test's runtime; the bridge runs on its own
//! thread, as in the app.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::{Client, ConnectOptions};
use massorca_desktop::service::{
    Refusal, ServiceBridge, ServiceHandle, StoreState, StoreUpdate, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::projects::{Project, ProjectAddParams, ProjectKind};
use massorca_server::{RunningService, Service, ServiceConfig};
use tempfile::TempDir;
use tokio::sync::mpsc::UnboundedReceiver;
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

/// A repository with one commit.
fn repo(dir: &Path) {
    std::fs::create_dir_all(dir).expect("the repository directory");
    git(dir, &["init", "-q"]);
    std::fs::write(dir.join("README.md"), "hello\n").expect("a readme");
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "initial"]);
}

/// The projects the service lists, read through a client of its own.
async fn listed_projects(home: &Path) -> Vec<Project> {
    let client = Client::connect(
        &profile(home),
        ConnectOptions::new(PeerInfo { ..base_peer_info() }),
    )
    .await
    .expect("a client connects");
    client
        .project_list()
        .await
        .expect("the projects are listed")
        .projects
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
            state.apply(update, Instant::now());
        }
    })
    .await;
    assert!(folded.is_ok(), "timed out waiting for {what}: {state:?}");
}

fn bridge_for(home: &Path) -> (ServiceBridge, ServiceHandle, UnboundedReceiver<StoreUpdate>) {
    let mut bridge =
        ServiceBridge::start(profile(home), connect_options(None)).expect("the bridge starts");
    let updates = bridge.take_updates().expect("the updates are there");
    let handle = bridge.handle();
    (bridge, handle, updates)
}

#[tokio::test]
async fn inspecting_a_path_adds_nothing_and_a_plain_folder_is_added_as_a_folder() {
    let home = temp_home();
    let repo_dir = home.path().join("demo");
    repo(&repo_dir);
    let notes = home.path().join("notes");
    std::fs::create_dir_all(&notes).expect("a plain folder");
    let service = start_service(home.path()).await;

    let (bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the service is connected", |s| {
        s.connection.is_connected() && s.loaded
    })
    .await;

    // Inspecting only describes: a repository is a Git project, a plain folder is a folder, and nothing is added.
    let repository = handle
        .inspect_project(repo_dir.display().to_string())
        .await
        .expect("the repository is inspected");
    assert_eq!(repository.kind, ProjectKind::Git);
    assert_eq!(repository.registered, None);
    let plain = handle
        .inspect_project(notes.display().to_string())
        .await
        .expect("the folder is inspected");
    assert_eq!(plain.kind, ProjectKind::Folder);
    assert_eq!(plain.registered, None);
    assert!(
        listed_projects(home.path()).await.is_empty(),
        "inspecting adds nothing"
    );

    // Continue without Git: the folder is registered as a folder, and inspecting it again says so.
    let added = handle
        .add_project(ProjectAddParams {
            path: notes.display().to_string(),
            kind: Some(ProjectKind::Folder),
            ..base_project_add_params()
        })
        .await
        .expect("the folder is added as a folder");
    assert_eq!(added.kind, ProjectKind::Folder);
    let again = handle
        .inspect_project(notes.display().to_string())
        .await
        .expect("the folder is inspected again");
    assert_eq!(again.registered.as_deref(), Some(added.id.as_str()));

    // A path the service cannot use is refused as a path problem, as the add refuses it.
    let missing = handle
        .inspect_project(home.path().join("missing").display().to_string())
        .await
        .expect_err("the missing path is refused");
    assert!(
        matches!(missing, Refusal::PathProblem { .. }),
        "got {missing:?}"
    );

    drop(bridge);
    stop_service(service).await;
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
