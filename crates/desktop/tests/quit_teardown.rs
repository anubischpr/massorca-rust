//! Quitting closes this computer's link to the service and leaves the service running. The service runs in this test's
//! runtime; the bridge runs on its own thread, and the link is closed from a thread outside any runtime, as the app does.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::{Client, ConnectOptions};
use massorca_desktop::service::{
    ConnectionState, QUIT_BOUND, ServiceBridge, close_link_within, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_server::{RunningService, Service, ServiceConfig};
use tempfile::TempDir;
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

fn peer() -> PeerInfo {
    PeerInfo { ..base_peer_info() }
}

/// Closes the link of `bridge` the way quit does, and reports whether it finished in time and how long that took.
fn quit_the_link(bridge: &ServiceBridge) -> (bool, Duration) {
    let closer = bridge.closer();
    std::thread::spawn(move || {
        let started = Instant::now();
        let closed = close_link_within(&closer, QUIT_BOUND);
        (closed, started.elapsed())
    })
    .join()
    .expect("the quit thread does not panic")
}

#[tokio::test]
async fn quitting_closes_the_link_and_the_service_keeps_running() {
    let home = temp_home();
    let running: RunningService = Service::start(ServiceConfig {
        profile: profile(home.path()),
        server_name: "massorca-test".to_owned(),
        server_version: "0.0.0-test".to_owned(),
    })
    .await
    .expect("the service starts");
    let bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let mut connection = bridge.handle().watch_connection();
    timeout(
        WAIT,
        connection.wait_for(|state| matches!(state, ConnectionState::Connected { .. })),
    )
    .await
    .expect("the bridge connects in time")
    .expect("the bridge is alive");

    let (closed, _) = quit_the_link(&bridge);
    assert!(closed, "the link finished within the quit bound");

    // A new connection is still served: quitting asked the service to do nothing.
    let client = timeout(
        WAIT,
        Client::connect(&profile(home.path()), ConnectOptions::new(peer())),
    )
    .await
    .expect("a client connects in time")
    .expect("the service still accepts connections");
    client
        .service_info()
        .await
        .expect("the service still answers");

    running.shutdown_handle().shutdown();
    timeout(WAIT, running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

#[tokio::test]
async fn quitting_while_the_service_is_down_does_not_wait_for_a_retry() {
    let home = temp_home();
    let bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");

    let (closed, elapsed) = quit_the_link(&bridge);

    assert!(closed, "the link finished within the quit bound");
    assert!(
        elapsed < QUIT_BOUND,
        "quitting waited for a retry: {elapsed:?}"
    );
}

/// Every `PeerInfo` field at its usual value; tests override the fields they vary.
fn base_peer_info() -> massorca_protocol::PeerInfo {
    massorca_protocol::PeerInfo {
        name: "massorca-test-client".to_owned(),
        version: "0.0.0-test".to_owned(),
    }
}
