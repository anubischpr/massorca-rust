//! The core settings (SET-01) through the bridge against a real service in a throwaway profile, driven headlessly: no
//! window opens. The store reads every setting when it connects, follows a write and a reset, and the reset's answer
//! is the default.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::{Client, ConnectOptions};
use massorca_desktop::service::{
    ServiceBridge, ServiceHandle, StoreState, StoreUpdate, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::config::{ConfigKey, ConfigSet};
use massorca_server::{RunningService, Service, ServiceConfig};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(30);

/// A private home below a temporary folder; the temporary folder itself may be shared with other users (ADR 0073).
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

fn peer() -> PeerInfo {
    PeerInfo {
        name: "massorca-settings-test".to_owned(),
        version: "0.0.0-test".to_owned(),
    }
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

fn held(state: &StoreState, key: &str) -> Option<Value> {
    state.config_value(key).map(|config| config.value.clone())
}

#[tokio::test]
async fn the_store_reads_the_settings_at_start_and_follows_a_write_and_a_reset() {
    const SHELL: &str = "terminal.shell";
    const SUMMARY: &str = "appearance.session_summary";
    let home = temp_home();
    let service = start_service(&home.path).await;

    // Another client stores a value before the desktop connects; the desktop reads it at start.
    let other = Client::connect(&profile(&home.path), ConnectOptions::new(peer()))
        .await
        .expect("a client connects");
    other
        .config_set(&ConfigSet {
            owner: None,
            key: SHELL.to_owned(),
            value: json!("pwsh"),
        })
        .await
        .expect("the shell is stored");
    drop(other);

    let (bridge, handle, mut updates) = bridge_for(&home.path);
    let mut state = StoreState::default();
    until(
        &mut updates,
        &mut state,
        "the settings read at start",
        |s| held(s, SHELL) == Some(json!("pwsh")) && held(s, SUMMARY).is_some(),
    )
    .await;
    assert_eq!(held(&state, SUMMARY), Some(json!(true)), "the default");

    handle
        .config_set(ConfigSet {
            owner: None,
            key: SUMMARY.to_owned(),
            value: json!(false),
        })
        .await
        .expect("the desktop's write is stored");
    until(&mut updates, &mut state, "the write", |s| {
        held(s, SUMMARY) == Some(json!(false))
    })
    .await;

    let reset = handle
        .config_reset(ConfigKey {
            owner: None,
            key: SUMMARY.to_owned(),
        })
        .await
        .expect("the reset is stored");
    assert_eq!(reset.value, json!(true));
    assert!(reset.is_default);
    until(&mut updates, &mut state, "the reset", |s| {
        s.config_value(SUMMARY)
            .is_some_and(|config| config.is_default && config.value == json!(true))
    })
    .await;

    // A reset of a key the service does not declare is refused, and the store keeps what it held.
    handle
        .config_reset(ConfigKey {
            owner: None,
            key: "no.such.key".to_owned(),
        })
        .await
        .expect_err("an unknown key is refused");

    drop(bridge);
    stop_service(service).await;
}
