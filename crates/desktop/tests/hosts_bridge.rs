//! Hosts, devices and pairing through the bridge, against real services on loopback (decisions 0041 and 0046).
//! Each computer is a temporary profile; nothing reaches a real service or a real host.

use std::net::{Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::{
    Backoff, ClientError, DeviceRevocation, HostDial, Hosts, LiveHost, LiveState,
    default_device_name,
};
use massorca_desktop::service::{
    HostLink, HostLoss, Refusal, ServiceBridge, ServiceHandle, SshHost, StoreState, StoreUpdate,
    connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::devices::{DeviceRevokeParams, PairingCreateParams};
use massorca_protocol::methods::projects::{ProjectAddParams, ProjectKind};
use massorca_server::{ListenSpec, NetworkConfig, RunningService, Service, ServiceConfig};
use tempfile::TempDir;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{sleep, timeout};

#[path = "support/host_proxy.rs"]
mod host_proxy;
use host_proxy::HostProxy;

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

/// A service that listens on an ephemeral loopback port, as a computer that others pair with.
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
            server_name: "massorca-hosts-test".to_owned(),
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

/// Waits until this service is connected to the bridge and its listener is bound, so an offer can name it.
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

fn is_connected(state: &StoreState, name: &str) -> bool {
    state
        .hosts
        .iter()
        .any(|entry| entry.record.name == name && matches!(entry.link, HostLink::Connected { .. }))
}

#[tokio::test]
async fn an_offer_pairs_this_computer_and_the_host_shows_connected() {
    let server_home = temp_home();
    let server = start_listening(server_home.path()).await;
    let (server_bridge, server_handle, mut server_updates) = bridge_for(server_home.path());
    let mut server_state = StoreState::default();
    ready_to_offer(&server_handle, &mut server_updates, &mut server_state).await;
    let offer = server_handle
        .create_offer(PairingCreateParams {
            ..base_pairing_create_params()
        })
        .await
        .expect("an offer is made");

    // This computer has no service of its own: accepting an offer needs only the network.
    let client_home = temp_home();
    let (client_bridge, client_handle, mut client_updates) = bridge_for(client_home.path());
    let record = client_handle
        .accept_offer(offer.offer.expose().to_owned(), Some("vps".to_owned()))
        .await
        .expect("the offer pairs");
    assert_eq!(record.name, "vps");
    assert_eq!(record.device_name, default_device_name());

    let mut state = StoreState::default();
    until(&mut client_updates, &mut state, "the paired host", |s| {
        is_connected(s, "vps")
    })
    .await;
    let entry = state.hosts.first().expect("the host is listed");
    assert!(matches!(
        &entry.link,
        HostLink::Connected { server, .. } if server == "massorca-hosts-test"
    ));
    assert!(entry.last_connected.is_some());

    drop(client_bridge);
    drop(server_bridge);
    stop_service(server).await;
}

#[tokio::test]
async fn a_host_that_stops_answering_is_unreachable_and_says_why() {
    let server_home = temp_home();
    let server = start_listening(server_home.path()).await;
    let (server_bridge, server_handle, mut server_updates) = bridge_for(server_home.path());
    let mut server_state = StoreState::default();
    ready_to_offer(&server_handle, &mut server_updates, &mut server_state).await;
    let offer = server_handle
        .create_offer(PairingCreateParams {
            ..base_pairing_create_params()
        })
        .await
        .expect("an offer is made");

    let client_home = temp_home();
    let (client_bridge, client_handle, mut client_updates) = bridge_for(client_home.path());
    client_handle
        .accept_offer(offer.offer.expose().to_owned(), Some("vps".to_owned()))
        .await
        .expect("the offer pairs");
    let mut state = StoreState::default();
    until(&mut client_updates, &mut state, "the paired host", |s| {
        is_connected(s, "vps")
    })
    .await;

    drop(server_bridge);
    stop_service(server).await;
    // The host is followed live: no refresh is needed to learn that it stopped answering.
    until(
        &mut client_updates,
        &mut state,
        "the unreachable host",
        |s| {
            s.hosts.iter().any(|entry| {
                matches!(
                    &entry.link,
                    HostLink::Reconnecting { reason, .. } if reason.starts_with("The host is unreachable: ")
                )
            })
        },
    )
    .await;
    let entry = state.hosts.first().expect("the host stays listed");
    assert!(
        entry.last_connected.is_some(),
        "the last time it answered is kept"
    );

    drop(client_bridge);
}

#[tokio::test]
async fn devices_are_listed_and_a_revoke_is_read_back_from_the_service() {
    let server_home = temp_home();
    let server = start_listening(server_home.path()).await;
    let (server_bridge, server_handle, mut server_updates) = bridge_for(server_home.path());
    let mut state = StoreState::default();
    ready_to_offer(&server_handle, &mut server_updates, &mut state).await;
    let offer = server_handle
        .create_offer(PairingCreateParams {
            ..base_pairing_create_params()
        })
        .await
        .expect("an offer is made");

    let client_home = temp_home();
    let (client_bridge, client_handle, _client_updates) = bridge_for(client_home.path());
    client_handle
        .accept_offer(offer.offer.expose().to_owned(), None)
        .await
        .expect("the offer pairs");

    let listed = server_handle
        .list_devices()
        .await
        .expect("the devices are listed");
    let device = listed
        .devices
        .iter()
        .find(|device| device.name == default_device_name())
        .expect("the paired device is listed")
        .clone();
    assert_eq!(device.revoked_at_ms, None);
    until(
        &mut server_updates,
        &mut state,
        "the device list",
        |s| matches!(&s.devices, Some(Ok(devices)) if devices.iter().any(|d| d.id == device.id)),
    )
    .await;

    let revoked = server_handle
        .revoke_device(DeviceRevokeParams {
            device: device.id.clone(),
        })
        .await
        .expect("the device is revoked");
    assert!(revoked.revoked_at_ms.is_some());
    until(&mut server_updates, &mut state, "the revoked device", |s| {
        matches!(&s.devices, Some(Ok(devices)) if devices.iter().any(|d| d.id == device.id && d.revoked_at_ms.is_some()))
    })
    .await;

    let missing = server_handle
        .revoke_device(DeviceRevokeParams {
            ..base_device_revoke_params()
        })
        .await
        .expect_err("an unknown device is refused");
    assert!(
        matches!(&missing, Refusal::Failed { code, .. } if code == "device.not_found"),
        "{missing:?}"
    );

    drop(client_bridge);
    drop(server_bridge);
    stop_service(server).await;
}

#[tokio::test]
async fn a_refused_offer_reaches_the_caller_as_the_service_sentence() {
    let server_home = temp_home();
    let server = start_listening(server_home.path()).await;
    let (server_bridge, server_handle, mut server_updates) = bridge_for(server_home.path());
    let mut state = StoreState::default();
    ready_to_offer(&server_handle, &mut server_updates, &mut state).await;
    let offer = server_handle
        .create_offer(PairingCreateParams {
            ..base_pairing_create_params()
        })
        .await
        .expect("an offer is made");
    let text = offer.offer.expose().to_owned();

    let client_home = temp_home();
    let (client_bridge, client_handle, _client_updates) = bridge_for(client_home.path());
    client_handle
        .accept_offer(text.clone(), Some("vps".to_owned()))
        .await
        .expect("the first redeem pairs");

    // The offer works once: a second redeem is refused by the server, with its own sentence.
    let second_home = temp_home();
    let (second_bridge, second_handle, _second_updates) = bridge_for(second_home.path());
    let refused = second_handle
        .accept_offer(text, Some("again".to_owned()))
        .await
        .expect_err("a used offer is refused");
    assert!(
        matches!(
            &refused,
            Refusal::Failed { code, message }
                if code == "pairing.invalid"
                    && message == "the pairing offer is not valid here; it may have been used already"
        ),
        "{refused:?}"
    );

    let not_an_offer = second_handle
        .accept_offer("not an offer".to_owned(), None)
        .await
        .expect_err("text that is not an offer is refused");
    assert!(
        matches!(&not_an_offer, Refusal::Failed { code, .. } if code == "pairing.not_an_offer"),
        "{not_an_offer:?}"
    );

    drop(second_bridge);
    drop(client_bridge);
    drop(server_bridge);
    stop_service(server).await;
}

#[tokio::test]
async fn an_ssh_add_that_is_refused_before_it_starts_keeps_an_empty_log() {
    let home = temp_home();
    let (bridge, handle, _updates) = bridge_for(home.path());
    let binary = home.path().join("massorca");
    std::fs::write(&binary, b"not a program").expect("a stand-in binary");

    let failed = handle
        .add_ssh_host(SshHost {
            target: "-oProxyCommand=echo".to_owned(),
            ssh_args: Vec::new(),
            binary,
            name: None,
            device_name: None,
        })
        .await
        .expect_err("a destination that starts with a dash is refused");
    assert!(
        matches!(
            &failed.refusal,
            Refusal::Failed { code, .. } if code == "usage.invalid_ssh_target" || code == "ssh.not_found"
        ),
        "{failed:?}"
    );
    assert!(failed.log.is_empty(), "{failed:?}");

    drop(bridge);
}

/// A computer that paired with the listening service as host `vps`, with its bridge and the store it folds.
async fn paired_with(
    server_handle: &ServiceHandle,
) -> (
    TempHome,
    ServiceBridge,
    ServiceHandle,
    UnboundedReceiver<StoreUpdate>,
    StoreState,
) {
    let offer = server_handle
        .create_offer(PairingCreateParams {
            ..base_pairing_create_params()
        })
        .await
        .expect("an offer is made");
    let client_home = temp_home();
    let (client_bridge, client_handle, mut client_updates) = bridge_for(client_home.path());
    client_handle
        .accept_offer(offer.offer.expose().to_owned(), Some("vps".to_owned()))
        .await
        .expect("the offer pairs");
    let mut state = StoreState::default();
    until(&mut client_updates, &mut state, "the paired host", |s| {
        is_connected(s, "vps")
    })
    .await;
    (
        client_home,
        client_bridge,
        client_handle,
        client_updates,
        state,
    )
}

#[tokio::test]
async fn removing_a_host_revokes_this_device_there_and_forgets_it_here() {
    let server_home = temp_home();
    let server = start_listening(server_home.path()).await;
    let (server_bridge, server_handle, mut server_updates) = bridge_for(server_home.path());
    let mut server_state = StoreState::default();
    ready_to_offer(&server_handle, &mut server_updates, &mut server_state).await;
    let (client_home, client_bridge, client_handle, mut client_updates, mut state) =
        paired_with(&server_handle).await;

    let removal = client_handle
        .remove_host("vps".to_owned())
        .await
        .expect("the host is removed");
    assert_eq!(removal.host, "vps");
    assert_eq!(removal.device, DeviceRevocation::Revoked);
    until(&mut client_updates, &mut state, "the forgotten host", |s| {
        s.hosts.is_empty()
    })
    .await;
    assert!(
        !Hosts::of(&profile(client_home.path())).exists("vps"),
        "the host record is gone from this computer"
    );
    let listed = server_handle
        .list_devices()
        .await
        .expect("the devices are listed");
    let device = listed
        .devices
        .iter()
        .find(|device| device.name == default_device_name())
        .expect("the device stays listed on the server, revoked");
    assert!(device.revoked_at_ms.is_some(), "{device:?}");

    let again = client_handle
        .remove_host("vps".to_owned())
        .await
        .expect_err("a host that is gone cannot be removed again");
    assert!(
        matches!(&again, Refusal::Failed { code, .. } if code == "host.not_found"),
        "{again:?}"
    );

    drop(client_bridge);
    drop(server_bridge);
    stop_service(server).await;
}

#[tokio::test]
async fn a_host_that_does_not_answer_is_still_forgotten_and_says_the_device_stays_paired() {
    let server_home = temp_home();
    let server = start_listening(server_home.path()).await;
    let (server_bridge, server_handle, mut server_updates) = bridge_for(server_home.path());
    let mut server_state = StoreState::default();
    ready_to_offer(&server_handle, &mut server_updates, &mut server_state).await;
    let (client_home, client_bridge, client_handle, _client_updates, _state) =
        paired_with(&server_handle).await;
    drop(server_bridge);
    stop_service(server).await;

    let removal = client_handle
        .remove_host("vps".to_owned())
        .await
        .expect("an unreachable host is still removed");
    assert!(
        matches!(&removal.device, DeviceRevocation::NotRevoked(reason) if !reason.is_empty()),
        "{removal:?}"
    );
    assert!(!Hosts::of(&profile(client_home.path())).exists("vps"));
    drop(client_bridge);
}

#[tokio::test]
async fn a_refresh_during_a_removal_does_not_bring_the_removed_host_back() {
    let server_home = temp_home();
    let server = start_listening(server_home.path()).await;
    let (server_bridge, server_handle, mut server_updates) = bridge_for(server_home.path());
    let mut server_state = StoreState::default();
    ready_to_offer(&server_handle, &mut server_updates, &mut server_state).await;
    let (client_home, client_bridge, client_handle, mut client_updates, _state) =
        paired_with(&server_handle).await;

    // The host answers nothing from now on, so the revoke waits out its connect timeout during the removal.
    let (_silent, address) = silent_address();
    readdress(client_home.path(), "vps", vec![address]);
    while client_updates.try_recv().is_ok() {}

    let removing = client_handle.remove_host("vps".to_owned());
    tokio::pin!(removing);
    let refresh = tokio::time::sleep(Duration::from_secs(1));
    tokio::pin!(refresh);
    let mut refreshed = false;
    // Updates sent while the removal runs are read and set aside: only what follows its answer is checked below.
    let removal = loop {
        tokio::select! {
            outcome = &mut removing => break outcome,
            () = &mut refresh, if !refreshed => {
                refreshed = true;
                client_handle.refresh_hosts();
            }
            _ = client_updates.recv() => {}
        }
    };
    removal.expect("the host is removed");

    // Once the removal has answered, nothing about the removed host arrives: its link and its follower are gone.
    let quiet = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(quiet);
    loop {
        tokio::select! {
            () = &mut quiet => break,
            update = client_updates.recv() => {
                let update = update.expect("the bridge is running");
                assert!(!concerns_host(&update, "vps"), "{update:?}");
            }
        }
    }
    drop(client_bridge);
    drop(server_bridge);
    stop_service(server).await;
}

#[tokio::test]
async fn the_remove_confirm_counts_the_projects_and_sessions_on_the_host() {
    let server_home = temp_home();
    let server = start_listening(server_home.path()).await;
    let (server_bridge, server_handle, mut server_updates) = bridge_for(server_home.path());
    let mut server_state = StoreState::default();
    ready_to_offer(&server_handle, &mut server_updates, &mut server_state).await;
    let folder = server_home.path().join("notes");
    std::fs::create_dir(&folder).expect("a project folder");
    server_handle
        .add_project(ProjectAddParams {
            path: folder.display().to_string(),
            name: Some("notes".to_owned()),
            kind: Some(ProjectKind::Folder),
            ..base_project_add_params()
        })
        .await
        .expect("the project is added");
    let (_client_home, client_bridge, client_handle, _client_updates, _state) =
        paired_with(&server_handle).await;

    let loss = client_handle
        .count_host_loss("vps".to_owned())
        .await
        .expect("the host answers");
    assert_eq!(
        loss,
        HostLoss {
            projects: 1,
            sessions: 0
        }
    );

    drop(server_bridge);
    stop_service(server).await;
    let unanswered = client_handle
        .count_host_loss("vps".to_owned())
        .await
        .expect_err("a host that does not answer cannot be counted");
    assert!(
        matches!(unanswered, Refusal::Unavailable { .. }),
        "{unanswered:?}"
    );
    drop(client_bridge);
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

/// Every `DeviceRevokeParams` field at its usual value; tests override the fields they vary.
fn base_device_revoke_params() -> massorca_protocol::methods::devices::DeviceRevokeParams {
    massorca_protocol::methods::devices::DeviceRevokeParams {
        device: "no-such-device".to_owned(),
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

#[tokio::test]
async fn a_paired_host_is_followed_live_while_it_stops_and_comes_back() {
    let server_home = temp_home();
    let server = start_listening(server_home.path()).await;
    let (server_bridge, server_handle, mut server_updates) = bridge_for(server_home.path());
    let mut server_state = StoreState::default();
    ready_to_offer(&server_handle, &mut server_updates, &mut server_state).await;
    let (client_home, client_bridge, client_handle, mut client_updates, mut state) =
        paired_with(&server_handle).await;
    let address = paired_address(client_home.path(), "vps");

    drop(server_bridge);
    stop_service(server).await;
    until(&mut client_updates, &mut state, "the lost host", |s| {
        s.hosts
            .iter()
            .any(|entry| matches!(entry.link, HostLink::Reconnecting { .. }))
    })
    .await;

    let server = start_listening_at(server_home.path(), address).await;
    // Retry now: the host is tried at once instead of after its backoff.
    client_handle.reconnect_host("vps".to_owned());
    until(&mut client_updates, &mut state, "the host back", |s| {
        is_connected(s, "vps")
    })
    .await;

    drop(client_bridge);
    stop_service(server).await;
}

#[tokio::test]
async fn a_bridge_follows_the_hosts_paired_before_it_started() {
    let server_home = temp_home();
    let server = start_listening(server_home.path()).await;
    let (server_bridge, server_handle, mut server_updates) = bridge_for(server_home.path());
    let mut server_state = StoreState::default();
    ready_to_offer(&server_handle, &mut server_updates, &mut server_state).await;
    let (client_home, client_bridge, _client_handle, _client_updates, _state) =
        paired_with(&server_handle).await;
    drop(client_bridge);

    // A new run of the desktop: no page asked for the hosts, and the paired host is connected all the same.
    let (bridge, _handle, mut updates) = bridge_for(client_home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the paired host", |s| {
        is_connected(s, "vps")
    })
    .await;

    drop(bridge);
    drop(server_bridge);
    stop_service(server).await;
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

/// Whether an update is about host `name`: its link or its sessions.
fn concerns_host(update: &StoreUpdate, name: &str) -> bool {
    match update {
        StoreUpdate::HostLinkChanged { name: host, .. } => host == name,
        StoreUpdate::HostSessions(read) => read.host == name,
        _ => false,
    }
}

/// An address that accepts TCP and never answers TLS, so each try waits out the whole connect timeout.
fn silent_address() -> (std::net::TcpListener, String) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a silent listener");
    let address = listener.local_addr().expect("its address").to_string();
    (listener, address)
}

/// The live link of host `name` on the computer at `home`, until its state satisfies `done`.
async fn follow_until(home: &Path, name: &str, what: &str, done: impl Fn(&LiveState) -> bool) {
    let peer = PeerInfo {
        name: "massorca-desktop-test".to_owned(),
        version: "0.0.0-test".to_owned(),
    };
    let link = LiveHost::start(
        HostDial::new(Hosts::of(&profile(home)), name, peer),
        Backoff::default(),
    );
    let mut states = link.subscribe();
    let reached = timeout(
        Duration::from_secs(45),
        states.wait_for(|state| done(state)),
    )
    .await
    .is_ok_and(|waited| waited.is_ok());
    assert!(
        reached,
        "timed out waiting for {what}: {:?}",
        *states.borrow()
    );
}

#[tokio::test]
async fn a_host_whose_first_address_is_silent_is_reached_at_the_next_one() {
    let server_home = temp_home();
    let server = start_listening(server_home.path()).await;
    let (server_bridge, server_handle, mut server_updates) = bridge_for(server_home.path());
    let mut server_state = StoreState::default();
    ready_to_offer(&server_handle, &mut server_updates, &mut server_state).await;
    let (client_home, client_bridge, _client_handle, _client_updates, _state) =
        paired_with(&server_handle).await;
    drop(client_bridge);
    let working = paired_address(client_home.path(), "vps").to_string();
    let (_silent, silent) = silent_address();
    readdress(client_home.path(), "vps", vec![silent, working]);

    follow_until(
        client_home.path(),
        "vps",
        "the second address",
        LiveState::is_connected,
    )
    .await;

    drop(server_bridge);
    stop_service(server).await;
}

#[tokio::test]
async fn another_key_at_the_next_address_is_a_mismatch_and_not_unreachable() {
    let server_home = temp_home();
    let server = start_listening(server_home.path()).await;
    let (server_bridge, server_handle, mut server_updates) = bridge_for(server_home.path());
    let mut server_state = StoreState::default();
    ready_to_offer(&server_handle, &mut server_updates, &mut server_state).await;
    let (client_home, client_bridge, _client_handle, _client_updates, _state) =
        paired_with(&server_handle).await;
    drop(client_bridge);
    // Another computer, with a key of its own, answers at the second address.
    let other_home = temp_home();
    let other = start_listening(other_home.path()).await;
    let (other_bridge, other_handle, mut other_updates) = bridge_for(other_home.path());
    let mut other_state = StoreState::default();
    ready_to_offer(&other_handle, &mut other_updates, &mut other_state).await;
    let other_address = other_handle
        .network_status()
        .await
        .expect("the other listener")
        .listeners[0]
        .bound
        .clone()
        .expect("the other listener is bound");
    let (_silent, silent) = silent_address();
    readdress(client_home.path(), "vps", vec![silent, other_address]);

    follow_until(client_home.path(), "vps", "the mismatch", |state| {
        matches!(state, LiveState::Ended { error } if matches!(**error, ClientError::ServerIdentityMismatch(_)))
    })
    .await;

    drop(other_bridge);
    stop_service(other).await;
    drop(server_bridge);
    stop_service(server).await;
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
async fn retry_now_reconnects_the_host_it_names_and_no_other() {
    let server_home = temp_home();
    let server = start_listening(server_home.path()).await;
    let (server_bridge, server_handle, mut server_updates) = bridge_for(server_home.path());
    let mut server_state = StoreState::default();
    ready_to_offer(&server_handle, &mut server_updates, &mut server_state).await;
    let (client_home, client_bridge, client_handle, _client_updates, _state) =
        paired_with(&server_handle).await;
    // The same server paired a second time, as a second host.
    let offer = server_handle
        .create_offer(PairingCreateParams {
            ..base_pairing_create_params()
        })
        .await
        .expect("a second offer is made");
    client_handle
        .accept_offer(offer.offer.expose().to_owned(), Some("nas".to_owned()))
        .await
        .expect("the second offer pairs");
    drop(client_bridge);
    let target = paired_address(client_home.path(), "vps");
    let vps = HostProxy::start(target).await;
    let nas = HostProxy::start(target).await;
    readdress(client_home.path(), "vps", vec![vps.address.to_string()]);
    readdress(client_home.path(), "nas", vec![nas.address.to_string()]);

    let (bridge, handle, mut updates) = bridge_for(client_home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "both hosts", |s| {
        is_connected(s, "vps") && is_connected(s, "nas")
    })
    .await;
    assert_eq!(
        (vps.accepted(), nas.accepted()),
        (1, 1),
        "one connection per host"
    );

    handle.reconnect_host("vps".to_owned());
    until_holds("the second connection to vps", || vps.accepted() == 2).await;
    until(&mut updates, &mut state, "vps back", |s| {
        is_connected(s, "vps")
    })
    .await;
    sleep(Duration::from_millis(500)).await;
    assert_eq!(nas.accepted(), 1, "nas is left alone");
    until_holds("the old connection to vps closed", || vps.open() == 1).await;
    assert_eq!(vps.accepted(), 2, "and no other was opened");

    drop(bridge);
    drop(server_bridge);
    stop_service(server).await;
}
