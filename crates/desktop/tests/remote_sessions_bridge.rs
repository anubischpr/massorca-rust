//! A paired host's agent sessions through the bridge (STAT-16), against two real services on loopback: one
//! computer runs a service with a session and is paired with by a second computer, which has no service of its
//! own. Each computer is a temporary profile; nothing reaches a real service or a real host.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_desktop::model::navigation::Mode;
use massorca_desktop::model::remote_sessions::remote_sessions;
use massorca_desktop::model::service_view::service_snapshot;
use massorca_desktop::model::shell::AgentState;
use massorca_desktop::model::sidebar::{SidebarContent, TreeRow, sidebar_content};
use massorca_desktop::service::{
    ServiceBridge, ServiceHandle, StoreState, StoreUpdate, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::methods::agents::{AgentSession, AgentStartParams};
use massorca_protocol::methods::devices::PairingCreateParams;
use massorca_protocol::methods::projects::ProjectAddParams;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};
use massorca_server::{ListenSpec, NetworkConfig, RunningService, Service, ServiceConfig};
use tempfile::TempDir;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{sleep, timeout};

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

async fn start_listening(home: &Path) -> RunningService {
    let network = NetworkConfig {
        listen: vec![ListenSpec::Address(SocketAddr::from((
            Ipv4Addr::LOCALHOST,
            0,
        )))],
    };
    Service::start_with_network(
        ServiceConfig {
            profile: profile(home),
            server_name: "massorca-remote-sessions-test".to_owned(),
            server_version: "0.0.0-test".to_owned(),
        },
        network,
    )
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
    let mut bridge =
        ServiceBridge::start(profile(home), connect_options(None)).expect("the bridge starts");
    let updates = bridge.take_updates().expect("the updates are there");
    let handle = bridge.handle();
    (bridge, handle, updates)
}

async fn until(
    updates: &mut UnboundedReceiver<StoreUpdate>,
    state: &mut StoreState,
    what: &str,
    done: impl Fn(&StoreState) -> bool,
) {
    until_within(updates, state, what, WAIT, done).await;
}

/// Folds the bridge's updates until `done` holds, or fails once `within` has passed.
async fn until_within(
    updates: &mut UnboundedReceiver<StoreUpdate>,
    state: &mut StoreState,
    what: &str,
    within: Duration,
    done: impl Fn(&StoreState) -> bool,
) {
    let folded = timeout(within, async {
        while !done(state) {
            let update = updates.recv().await.expect("the bridge is alive");
            state.apply(update, Instant::now());
        }
    })
    .await;
    assert!(folded.is_ok(), "timed out waiting for {what}: {state:?}");
}

/// Waits until the service is connected and listening, so an offer can name it.
async fn ready_to_offer(
    handle: &ServiceHandle,
    updates: &mut UnboundedReceiver<StoreUpdate>,
    state: &mut StoreState,
) {
    until(updates, state, "the connection", |s| {
        s.connection.is_connected()
    })
    .await;
    for _ in 0..200 {
        if let Ok(status) = handle.network_status().await
            && !status.listeners.is_empty()
            && status
                .listeners
                .iter()
                .all(|listener| listener.state == "listening")
        {
            return;
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("the network listener did not start");
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
            title: Some("Host session".to_owned()),
            cols: Some(100),
            rows: Some(30),
            request_id: None,
            permission_mode: None,
            model: None,
            effort: None,
            fast: None,
        })
        .await
        .expect("the session starts")
}

/// The ids of the sessions the Sidebar lists under `host`, in order.
fn listed_under(state: &StoreState, host: &str) -> Vec<String> {
    remote_sessions(state)
        .into_iter()
        .find(|(name, _)| name == host)
        .map(|(_, sessions)| sessions.rows.iter().map(|row| row.id.clone()).collect())
        .unwrap_or_default()
}

#[tokio::test]
async fn a_paired_hosts_session_is_listed_and_kept_unverifiable_when_the_host_stops() {
    let server_home = temp_home();
    let repo_dir = server_home.path.with_file_name("repo");
    repo(&repo_dir);
    declare_fake_kind(&server_home.path);
    let server = start_listening(&server_home.path).await;
    let (server_bridge, server_handle, mut server_updates) = bridge_for(&server_home.path);
    let mut server_state = StoreState::default();
    ready_to_offer(&server_handle, &mut server_updates, &mut server_state).await;
    let workspace = add_workspace(&server_handle, &repo_dir).await;
    let session = start_fake(&server_handle, &workspace.id).await;
    let offer = server_handle
        .create_offer(PairingCreateParams {
            ttl_secs: None,
            addresses: Vec::new(),
            admin: false,
            standard: false,
        })
        .await
        .expect("an offer is made");

    // This computer has no service of its own: it pairs with the server and reads its sessions over that link.
    let client_home = temp_home();
    let (client_bridge, client_handle, mut client_updates) = bridge_for(&client_home.path);
    client_handle
        .accept_offer(offer.offer.expose().to_owned(), Some("vps".to_owned()))
        .await
        .expect("the offer pairs");
    // Pairing does not check the host on its own; a check reads its sessions on the same connection.
    client_handle.refresh_hosts();
    let mut state = StoreState::default();
    let session_id = session.id.clone();
    until(&mut client_updates, &mut state, "the host's session", |s| {
        s.host_sessions.get("vps").is_some_and(|held| {
            held.reached && held.sessions.iter().any(|listed| listed.id == session_id)
        })
    })
    .await;
    assert_eq!(listed_under(&state, "vps"), vec![session.id.clone()]);
    assert!(listed_under(&state, "missing").is_empty());

    // The host's sessions sit under its row only: This PC's rows do not list them.
    let snapshot = service_snapshot(&state, Instant::now());
    let SidebarContent::Workspaces { tree, .. } = sidebar_content(&snapshot, Mode::Workspaces)
    else {
        panic!("the workspaces sidebar is shown");
    };
    assert!(
        !tree
            .iter()
            .any(|row| matches!(row, TreeRow::Session { id, .. } if id == &session.id)),
        "the host's session is not in This PC's rows"
    );

    // The host stops: the next check does not reach it, and its session stays listed, unverifiable.
    drop(server_bridge);
    stop_service(server).await;
    client_handle.refresh_hosts();
    until(&mut client_updates, &mut state, "the host unreached", |s| {
        s.host_sessions.get("vps").is_some_and(|held| !held.reached)
    })
    .await;
    assert_eq!(listed_under(&state, "vps"), vec![session.id.clone()]);
    let listed = remote_sessions(&state);
    let (_, sessions) = &listed[0];
    assert!(!sessions.reached);
    assert_eq!(
        sessions.rows[0].state,
        AgentState::Unreachable,
        "a host that cannot be reached is unverifiable, never exited"
    );

    drop(client_bridge);
}

/// The bridge's poll checks a host every 30 s; a change inside this window reached the desktop by the host's
/// event subscription, not by the poll.
const BEFORE_THE_POLL: Duration = Duration::from_secs(10);

#[tokio::test]
async fn a_session_started_on_the_host_is_listed_by_its_event_before_the_poll() {
    let server_home = temp_home();
    let repo_dir = server_home.path.with_file_name("repo");
    repo(&repo_dir);
    declare_fake_kind(&server_home.path);
    let server = start_listening(&server_home.path).await;
    let (server_bridge, server_handle, mut server_updates) = bridge_for(&server_home.path);
    let mut server_state = StoreState::default();
    ready_to_offer(&server_handle, &mut server_updates, &mut server_state).await;
    let workspace = add_workspace(&server_handle, &repo_dir).await;
    let first = start_fake(&server_handle, &workspace.id).await;
    let offer = server_handle
        .create_offer(PairingCreateParams {
            ttl_secs: None,
            addresses: Vec::new(),
            admin: false,
            standard: false,
        })
        .await
        .expect("an offer is made");

    let client_home = temp_home();
    let (client_bridge, client_handle, mut client_updates) = bridge_for(&client_home.path);
    client_handle
        .accept_offer(offer.offer.expose().to_owned(), Some("vps".to_owned()))
        .await
        .expect("the offer pairs");
    client_handle.refresh_hosts();
    let mut state = StoreState::default();
    let first_id = first.id.clone();
    until(&mut client_updates, &mut state, "the first session", |s| {
        listed_under(s, "vps").contains(&first_id)
    })
    .await;

    // The host starts a second session; the desktop follows the host's event and lists it without a refresh.
    // One workspace runs one writing session at a time, so the second session gets its own workspace.
    let second_workspace = server_handle
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
    let second = start_fake(&server_handle, &second_workspace.id).await;
    let second_id = second.id.clone();
    until_within(
        &mut client_updates,
        &mut state,
        "the second session by event",
        BEFORE_THE_POLL,
        |s| listed_under(s, "vps").contains(&second_id),
    )
    .await;

    drop(client_bridge);
    drop(server_bridge);
    stop_service(server).await;
}

#[tokio::test]
async fn a_stop_of_a_paired_hosts_session_is_answered_by_that_host() {
    let server_home = temp_home();
    let repo_dir = server_home.path.with_file_name("repo");
    repo(&repo_dir);
    declare_fake_kind(&server_home.path);
    let server = start_listening(&server_home.path).await;
    let (server_bridge, server_handle, mut server_updates) = bridge_for(&server_home.path);
    let mut server_state = StoreState::default();
    ready_to_offer(&server_handle, &mut server_updates, &mut server_state).await;
    let workspace = add_workspace(&server_handle, &repo_dir).await;
    let session = start_fake(&server_handle, &workspace.id).await;
    let offer = server_handle
        .create_offer(PairingCreateParams {
            ttl_secs: None,
            addresses: Vec::new(),
            admin: false,
            standard: false,
        })
        .await
        .expect("an offer is made");

    let client_home = temp_home();
    let (client_bridge, client_handle, mut client_updates) = bridge_for(&client_home.path);
    client_handle
        .accept_offer(offer.offer.expose().to_owned(), Some("vps".to_owned()))
        .await
        .expect("the offer pairs");
    client_handle.refresh_hosts();
    let mut state = StoreState::default();
    let session_id = session.id.clone();
    until(&mut client_updates, &mut state, "the host's session", |s| {
        listed_under(s, "vps").contains(&session_id)
    })
    .await;

    // The stop goes over the host's connection; the host answers with its own session.
    let stopped = client_handle
        .host_agent_stop("vps", &session.id)
        .await
        .expect("the host stops its session");
    assert_eq!(stopped.id, session.id);
    assert!(
        client_handle
            .host_agent_stop("missing", &session.id)
            .await
            .is_err(),
        "a host that is not paired is refused, not sent to another host"
    );

    drop(client_bridge);
    drop(server_bridge);
    stop_service(server).await;
}
