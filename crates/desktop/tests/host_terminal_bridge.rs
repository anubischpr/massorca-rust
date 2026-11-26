//! A paired host's terminal through the bridge (STAT-16), against two real services on loopback: one computer runs a
//! service with a session, and a second computer, which has no service of its own, pairs with it and attaches to the
//! session's terminal over the host's own connection. Each computer is a temporary profile; nothing reaches a real
//! service or a real host.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::AttachUpdate;
use massorca_desktop::service::{
    HostTerminal, HostTerminalItem, ServiceBridge, ServiceHandle, StoreState, StoreUpdate,
    connect_options,
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
            server_name: "massorca-host-terminal-test".to_owned(),
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

/// Folds the bridge's updates until `done` holds, or fails once WAIT has passed.
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

/// Waits until the service is connected and its listener is up, so an offer can name it.
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

/// Declares kind `fake`, which runs the platform shell, so its terminal echoes what is typed (see remote_sessions_bridge.rs).
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

/// The terminal of session `id` as `host` lists it, once the host has been read.
fn listed_terminal(state: &StoreState, host: &str, id: &str) -> Option<String> {
    let held = state.host_sessions.get(host)?;
    if !held.reached {
        return None;
    }
    held.sessions
        .iter()
        .find(|session| session.id == id)?
        .terminal_id
        .clone()
}

/// This computer, paired with a host that runs one session with a terminal. The host's service is `server`.
struct Pair {
    _server_home: TempHome,
    _client_home: TempHome,
    server: Option<RunningService>,
    server_bridge: Option<ServiceBridge>,
    /// A bridge stops once its updates are dropped, so each one is kept for as long as the pair lives.
    _server_updates: UnboundedReceiver<StoreUpdate>,
    client_bridge: ServiceBridge,
    _client_updates: UnboundedReceiver<StoreUpdate>,
    client: ServiceHandle,
    terminal: String,
}

impl Pair {
    /// Stops the host the way a host goes away: its bridge and its service end.
    async fn stop_host(&mut self) {
        drop(self.server_bridge.take());
        stop_service(self.server.take().expect("the host runs")).await;
    }
}

async fn pair() -> Pair {
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
    let (client_bridge, client, mut updates) = bridge_for(&client_home.path);
    client
        .accept_offer(offer.offer.expose().to_owned(), Some("vps".to_owned()))
        .await
        .expect("the offer pairs");
    client.refresh_hosts();
    let mut state = StoreState::default();
    let session_id = session.id.clone();
    until(&mut updates, &mut state, "the host's terminal", |s| {
        listed_terminal(s, "vps", &session_id).is_some()
    })
    .await;
    let terminal = listed_terminal(&state, "vps", &session_id).expect("the session has a terminal");
    Pair {
        _server_home: server_home,
        _client_home: client_home,
        server: Some(server),
        server_bridge: Some(server_bridge),
        _server_updates: server_updates,
        client_bridge,
        _client_updates: updates,
        client,
        terminal,
    }
}

/// Reads the attach's items into `screen` until `done` holds on the screen, or fails once WAIT has passed. Items that
/// are not output are returned for the caller to check.
async fn read_until(
    attach: &mut HostTerminal,
    screen: &mut Vec<u8>,
    what: &str,
    done: impl Fn(&[u8]) -> bool,
) -> Vec<HostTerminalItem> {
    let mut others = Vec::new();
    let read = timeout(WAIT, async {
        while !done(screen) {
            match attach.next().await {
                Some(HostTerminalItem::Update(
                    AttachUpdate::Snapshot { bytes, .. } | AttachUpdate::Output { bytes, .. },
                )) => screen.extend(bytes),
                // The header of an attach or a size change shows nothing by itself.
                Some(HostTerminalItem::Update(_)) => {}
                Some(item) => others.push(item),
                None => panic!("the attach ended while {what}: {others:?}"),
            }
        }
    })
    .await;
    assert!(
        read.is_ok(),
        "timed out waiting for {what}: {}",
        String::from_utf8_lossy(screen)
    );
    others
}

fn shows(screen: &[u8], text: &[u8]) -> bool {
    screen.windows(text.len()).any(|window| window == text)
}

#[tokio::test]
async fn a_paired_hosts_terminal_shows_its_screen_and_echoes_what_is_typed() {
    let mut pair = pair().await;
    let mut attach = pair
        .client
        .attach_host_terminal("vps", &pair.terminal, 100, 30);
    let writer = attach.writer();
    let mut screen = Vec::new();
    read_until(&mut attach, &mut screen, "the host's screen", |screen| {
        !screen.is_empty()
    })
    .await;

    // Typed text goes to the host's terminal, which echoes it back on the same connection.
    writer.input(b"probe-7f3a".to_vec());
    let others = read_until(
        &mut attach,
        &mut screen,
        "the typed text echoed",
        |screen| shows(screen, b"probe-7f3a"),
    )
    .await;
    assert!(others.is_empty(), "only output came back: {others:?}");
    drop(attach);
    pair.stop_host().await;
    drop(pair.client_bridge);
}

#[tokio::test]
async fn a_host_that_stops_is_reported_out_of_contact_and_its_terminal_is_not_ended() {
    let mut pair = pair().await;
    let mut attach = pair
        .client
        .attach_host_terminal("vps", &pair.terminal, 100, 30);
    let mut screen = Vec::new();
    read_until(&mut attach, &mut screen, "the host's screen", |screen| {
        !screen.is_empty()
    })
    .await;

    pair.stop_host().await;
    // Output the host sent before it stopped may still come first; the loss follows it.
    let reported = timeout(WAIT, async {
        loop {
            match attach.next().await.expect("the attach stays open") {
                HostTerminalItem::Update(_) => {}
                item => break item,
            }
        }
    })
    .await
    .expect("the host's loss is reported");
    assert_eq!(
        reported,
        HostTerminalItem::Unreachable,
        "a lost host is out of contact, never an exit"
    );
    // The attach keeps trying in the background: no later item says the terminal ended or was refused.
    assert!(
        timeout(Duration::from_secs(2), attach.next())
            .await
            .is_err(),
        "nothing ends the attach while the host is out of contact"
    );
    assert!(!screen.is_empty(), "the last screen is kept");
    drop(attach);
    drop(pair.client_bridge);
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
    }
}
