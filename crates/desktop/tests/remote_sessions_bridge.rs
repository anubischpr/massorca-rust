//! A paired host's agent sessions through the bridge (STAT-16), against two real services on loopback: one
//! computer runs a service with a session and is paired with by a second computer, which has no service of its
//! own. Each computer is a temporary profile; nothing reaches a real service or a real host.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::Hosts;
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

#[path = "support/host_proxy.rs"]
mod host_proxy;
use host_proxy::HostProxy;

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
    start_listening_at(home, SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await
}

/// A service that listens on `address`, as a host coming back where it was paired.
async fn start_listening_at(home: &Path, address: SocketAddr) -> RunningService {
    let network = NetworkConfig {
        listen: vec![ListenSpec::Address(address)],
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
            path: repo_dir.display().to_string(),
            name: Some("demo".to_owned()),
            workspaces_root: repo_dir
                .parent()
                .map(|parent| parent.join("demo-worktrees").display().to_string()),
            ..base_project_add_params()
        })
        .await
        .expect("the project is added");
    handle
        .create_workspace(WorkspaceCreateParams {
            in_place: false,
            project: project.id,
            ..base_workspace_create_params()
        })
        .await
        .expect("the workspace is created")
}

async fn start_fake(handle: &ServiceHandle, workspace: &str) -> AgentSession {
    handle
        .agent_start(AgentStartParams {
            workspace: Some(workspace.to_owned()),
            env: BTreeMap::new(),
            title: Some("Host session".to_owned()),
            cols: Some(100),
            rows: Some(30),
            ..base_agent_start_params()
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
            ..base_pairing_create_params()
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
    // The host's live link reads its sessions on its connection; nothing needs to ask.
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

    // The host stops: its link loses the connection, and its session stays listed, unverifiable.
    let address = paired_address(&client_home.path, "vps");
    drop(server_bridge);
    stop_service(server).await;
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

    // The host comes back where it was: once its link reconnects, its sessions are read again and live.
    let server = start_listening_at(&server_home.path, address).await;
    client_handle.reconnect_host("vps".to_owned());
    until(
        &mut client_updates,
        &mut state,
        "the host reached again",
        |s| s.host_sessions.get("vps").is_some_and(|held| held.reached),
    )
    .await;
    assert!(
        listed_under(&state, "vps").contains(&session.id),
        "the host's session is listed again"
    );

    drop(client_bridge);
    stop_service(server).await;
}

/// Nothing reads a followed host on a timer; a change inside this window reached the desktop by the host's event.
const BY_THE_EVENT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn a_session_started_on_the_host_is_listed_by_its_event() {
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
            ..base_pairing_create_params()
        })
        .await
        .expect("an offer is made");

    let client_home = temp_home();
    let (client_bridge, client_handle, mut client_updates) = bridge_for(&client_home.path);
    client_handle
        .accept_offer(offer.offer.expose().to_owned(), Some("vps".to_owned()))
        .await
        .expect("the offer pairs");
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
            in_place: false,
            project: workspace.project_id.clone(),
            name: "second".to_owned(),
            ..base_workspace_create_params()
        })
        .await
        .expect("the second workspace is created");
    let second = start_fake(&server_handle, &second_workspace.id).await;
    let second_id = second.id.clone();
    until_within(
        &mut client_updates,
        &mut state,
        "the second session by event",
        BY_THE_EVENT,
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
            ..base_pairing_create_params()
        })
        .await
        .expect("an offer is made");

    let client_home = temp_home();
    let (client_bridge, client_handle, mut client_updates) = bridge_for(&client_home.path);
    client_handle
        .accept_offer(offer.offer.expose().to_owned(), Some("vps".to_owned()))
        .await
        .expect("the offer pairs");
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

/// Every `AgentStartParams` field at its usual value; tests override the fields they vary.
fn base_agent_start_params() -> massorca_protocol::methods::agents::AgentStartParams {
    massorca_protocol::methods::agents::AgentStartParams {
        workspace: None,
        kind: "fake".to_owned(),
        prompt: None,
        args: Vec::new(),
        env: Default::default(),
        read_only: false,
        title: None,
        cols: None,
        rows: None,
        request_id: None,
        permission_mode: None,
        model: None,
        effort: None,
        fast: None,
    }
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

/// Every `PairingCreateParams` field at its usual value; tests override the fields they vary.
fn base_pairing_create_params() -> massorca_protocol::methods::devices::PairingCreateParams {
    massorca_protocol::methods::devices::PairingCreateParams {
        ttl_secs: None,
        admin: false,
        standard: false,
        addresses: Vec::new(),
    }
}

/// The address the paired host `name` was reached at, so a restarted service can listen there again.
fn paired_address(home: &Path, name: &str) -> SocketAddr {
    let record = Hosts::of(&profile(home))
        .get(name)
        .expect("the host is paired");
    record
        .addresses
        .iter()
        .find_map(|address| address.to_socket_addrs().ok()?.next())
        .expect("the host has an address")
}

/// Stores host `name` again with `addresses` in place of the ones it was paired at, keeping its key.
fn readdress(home: &Path, name: &str, addresses: Vec<String>) {
    let hosts = Hosts::of(&profile(home));
    let mut record = hosts.get(name).expect("the host is paired");
    let identity = hosts.identity(name).expect("its device key");
    hosts.remove(name).expect("the old record goes");
    record.addresses = addresses;
    hosts
        .add(&record, &identity)
        .expect("the new record is stored");
}

/// Waits until `holds`, or fails once WAIT has passed.
async fn until_holds(what: &str, holds: impl Fn() -> bool) {
    let reached = timeout(WAIT, async {
        while !holds() {
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(reached.is_ok(), "timed out waiting for {what}");
}

#[tokio::test]
async fn one_connection_serves_a_hosts_sessions_and_calls_until_the_host_is_removed() {
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
            ..base_pairing_create_params()
        })
        .await
        .expect("an offer is made");
    let client_home = temp_home();
    let (client_bridge, client_handle, _client_updates) = bridge_for(&client_home.path);
    client_handle
        .accept_offer(offer.offer.expose().to_owned(), Some("vps".to_owned()))
        .await
        .expect("the offer pairs");
    drop(client_bridge);
    // The host is reached through a relay that counts this computer's connections to it.
    let relay = HostProxy::start(paired_address(&client_home.path, "vps")).await;
    readdress(&client_home.path, "vps", vec![relay.address.to_string()]);

    let (bridge, handle, mut updates) = bridge_for(&client_home.path);
    let mut state = StoreState::default();
    let session_id = session.id.clone();
    until(&mut updates, &mut state, "the host's session", |s| {
        s.host_sessions
            .get("vps")
            .is_some_and(|held| held.reached && held.sessions.iter().any(|l| l.id == session_id))
    })
    .await;
    // A call to the host, and the re-read its change starts, go over the same connection.
    let loss = handle
        .count_host_loss("vps".to_owned())
        .await
        .expect("the host counts what it runs");
    assert_eq!(loss.sessions, 1);
    let stopped = handle
        .host_agent_stop("vps", &session.id)
        .await
        .expect("the host stops its session");
    assert_ne!(
        stopped.execution, session.execution,
        "the stop changed the session"
    );
    until(
        &mut updates,
        &mut state,
        "the stop read again by its event",
        |s| {
            s.host_sessions.get("vps").is_some_and(|held| {
                held.sessions
                    .iter()
                    .any(|listed| listed.id == session_id && listed.execution != session.execution)
            })
        },
    )
    .await;
    assert_eq!(relay.accepted(), 1, "one connection to the host, shared");

    // Removing the host stops its link and its follower: the connection closes and none is opened again.
    handle
        .remove_host("vps".to_owned())
        .await
        .expect("the host is removed");
    until_holds("the host's connection closed", || relay.open() == 0).await;
    // The revoke dials on its own (once, or twice to confirm); after it nothing does.
    let dialed = relay.accepted();
    sleep(Duration::from_secs(1)).await;
    assert_eq!(relay.accepted(), dialed, "nothing dials a forgotten host");

    drop(bridge);
    drop(server_bridge);
    stop_service(server).await;
}
