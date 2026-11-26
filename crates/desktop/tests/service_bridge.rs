//! The bridge against a real service in a temporary profile, driven headlessly: no window opens.
//! The service runs in this test's runtime; the bridge runs on its own thread, as in the app.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::{Client, ConnectOptions};
use massorca_desktop::service::{
    ConnectionState, Refusal, ServiceBridge, ServiceHandle, StoreState, StoreUpdate,
    connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::methods::config::ConfigSet;
use massorca_protocol::methods::projects::ProjectAddParams;
use massorca_protocol::methods::workspaces::{WorkspaceCreateParams, WorkspaceRemoveParams};
use massorca_protocol::{PeerInfo, codes};
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

/// A repository with one commit and a `.gitignore` that ignores `*.log`.
fn repo(dir: &Path) {
    std::fs::create_dir_all(dir).expect("the repository directory");
    git(dir, &["init", "-q"]);
    std::fs::write(dir.join("README.md"), "hello\n").expect("a readme");
    std::fs::write(dir.join(".gitignore"), "*.log\n").expect("a gitignore");
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "initial"]);
}

/// Registers the repository as a project through a client of its own, as the CLI would.
async fn add_project(home: &Path, repo_dir: &Path) -> String {
    let client = Client::connect(
        &profile(home),
        ConnectOptions::new(PeerInfo {
            name: "massorca-test-client".to_owned(),
            version: "0.0.0-test".to_owned(),
        }),
    )
    .await
    .expect("a client connects");
    client
        .project_add(&ProjectAddParams {
            icon: None,
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
        .expect("the project is added")
        .id
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

fn create_params(project: &str, name: &str) -> WorkspaceCreateParams {
    WorkspaceCreateParams {
        project: project.to_owned(),
        name: name.to_owned(),
        branch: None,
        base: None,
        existing_branch: false,
        refresh_base: false,
        path: None,
        request_id: None,
        no_setup: false,
    }
}

fn remove_params(workspace: &str) -> WorkspaceRemoveParams {
    WorkspaceRemoveParams {
        workspace: workspace.to_owned(),
        force: false,
        discard_ignored: false,
        forget_orphan: false,
        with_children: false,
        allow_failed_archive: false,
    }
}

#[tokio::test]
async fn the_store_follows_the_service_and_waivers_retry_with_one_more_flag() {
    let home = temp_home();
    let repo_dir = home.path().join("demo");
    repo(&repo_dir);
    let service = start_service(home.path()).await;
    let project_id = add_project(home.path(), &repo_dir).await;

    let (bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the first full list", |s| {
        s.connection.is_connected() && s.loaded && s.project(&project_id).is_some()
    })
    .await;

    // Create: the answer does not touch the store; the event does.
    let created = handle
        .create_workspace(create_params(&project_id, "dirty-one"))
        .await
        .expect("the workspace is created");
    until(&mut updates, &mut state, "the created event", |s| {
        s.workspace(&created.id).is_some()
    })
    .await;

    // A name already taken is refused as a name problem, kept for the name field.
    let taken = handle
        .create_workspace(create_params(&project_id, "dirty-one"))
        .await
        .expect_err("the name is taken");
    assert!(
        matches!(taken, Refusal::NameProblem { .. }),
        "got {taken:?}"
    );

    // Remove with uncommitted work: refused as dirty, then allowed with exactly `force`.
    std::fs::write(Path::new(&created.path).join("scratch.txt"), "unsaved\n")
        .expect("an untracked file");
    let refused = handle
        .remove_workspace(remove_params(&created.id))
        .await
        .expect_err("a dirty workspace is not removed");
    assert_eq!(refused, Refusal::Dirty);
    assert!(
        state.workspace(&created.id).is_some(),
        "a refusal changes nothing in the store"
    );
    let forced = WorkspaceRemoveParams {
        force: true,
        ..remove_params(&created.id)
    };
    handle
        .remove_workspace(forced)
        .await
        .expect("force discards the changes");
    until(&mut updates, &mut state, "the removed event", |s| {
        s.workspace(&created.id).is_none()
    })
    .await;

    // Ignored files have their own waiver, asked for separately from `force`.
    let second = handle
        .create_workspace(create_params(&project_id, "ignored-one"))
        .await
        .expect("a second workspace");
    std::fs::write(Path::new(&second.path).join("build.log"), "noise\n").expect("an ignored file");
    let refused = handle
        .remove_workspace(remove_params(&second.id))
        .await
        .expect_err("ignored files are not deleted unseen");
    assert_eq!(refused, Refusal::IgnoredFiles);
    handle
        .remove_workspace(WorkspaceRemoveParams {
            discard_ignored: true,
            ..remove_params(&second.id)
        })
        .await
        .expect("discard_ignored lets it go");
    until(&mut updates, &mut state, "the second removal", |s| {
        s.workspaces.is_empty()
    })
    .await;

    drop(handle);
    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn an_absent_service_is_unreachable_until_retry_now_finds_it() {
    let home = temp_home();
    let (bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the unreachable state", |s| {
        matches!(s.connection, ConnectionState::Unreachable { .. })
    })
    .await;

    // A call while down is refused at once, not queued.
    let refused = handle
        .create_workspace(create_params("nope", "w"))
        .await
        .expect_err("nothing to ask");
    assert!(
        matches!(refused, Refusal::Unavailable { .. }),
        "{refused:?}"
    );

    let service = start_service(home.path()).await;
    handle.retry_now();
    until(&mut updates, &mut state, "the connection", |s| {
        s.connection.is_connected() && s.loaded
    })
    .await;
    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn a_lost_connection_reconnects_and_resyncs() {
    let home = temp_home();
    let repo_dir = home.path().join("demo");
    repo(&repo_dir);
    let service = start_service(home.path()).await;
    let project_id = add_project(home.path(), &repo_dir).await;
    let (bridge, _handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the first connection", |s| {
        s.connection.is_connected() && s.project(&project_id).is_some()
    })
    .await;

    stop_service(service).await;
    until(&mut updates, &mut state, "reconnecting", |s| {
        matches!(s.connection, ConnectionState::Reconnecting { .. })
    })
    .await;
    assert!(
        state.project(&project_id).is_some(),
        "the last known state stays while the link is down"
    );

    let again = start_service(home.path()).await;
    until(&mut updates, &mut state, "the reconnection", |s| {
        s.connection.is_connected()
    })
    .await;
    assert!(
        state.project(&project_id).is_some(),
        "resynced from the list"
    );
    drop(bridge);
    stop_service(again).await;
}

#[tokio::test]
async fn config_values_are_read_and_stored_through_the_bridge() {
    const FOLDER: &str = "workspaces.default_root";
    let home = temp_home();
    let service = start_service(home.path()).await;
    let (bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the connection", |s| {
        s.connection.is_connected()
    })
    .await;

    let listed = handle.config_list().await.expect("the settings are listed");
    let folder = listed
        .values
        .iter()
        .find(|value| value.key == FOLDER)
        .expect("the folder setting is declared");
    assert!(folder.is_default);
    let default_text = folder.value.as_str().expect("a path").to_owned();

    // The service refuses a folder that is not absolute, and the refusal keeps its code.
    let refused = handle
        .config_set(ConfigSet {
            owner: None,
            key: FOLDER.to_owned(),
            value: "relative".to_owned().into(),
        })
        .await
        .expect_err("a relative folder is refused");
    assert!(
        matches!(&refused, Refusal::Failed { code, .. } if code == codes::CONFIG_INVALID_VALUE),
        "{refused:?}"
    );

    let unknown = handle
        .config_set(ConfigSet {
            owner: None,
            key: "no.such.key".to_owned(),
            value: "x".to_owned().into(),
        })
        .await
        .expect_err("an unknown key is refused");
    assert!(
        matches!(&unknown, Refusal::Failed { code, .. } if code == codes::CONFIG_UNKNOWN_KEY),
        "{unknown:?}"
    );

    // An absolute folder is stored, and the answer is the value in effect.
    let chosen = std::env::temp_dir()
        .join("massorca-config-test")
        .to_string_lossy()
        .into_owned();
    let stored = handle
        .config_set(ConfigSet {
            owner: None,
            key: FOLDER.to_owned(),
            value: chosen.clone().into(),
        })
        .await
        .expect("an absolute folder is stored");
    assert_eq!(stored.value.as_str(), Some(chosen.as_str()));
    assert!(!stored.is_default);
    let relisted = handle
        .config_list()
        .await
        .expect("the settings are listed again");
    let again = relisted
        .values
        .iter()
        .find(|value| value.key == FOLDER)
        .expect("the folder setting is still declared");
    assert_eq!(again.value.as_str(), Some(chosen.as_str()));

    // Storing the default again makes it the default in effect.
    let back = handle
        .config_set(ConfigSet {
            owner: None,
            key: FOLDER.to_owned(),
            value: default_text.into(),
        })
        .await
        .expect("the default is stored");
    assert!(back.is_default);

    drop(bridge);
    stop_service(service).await;
}
