//! The secret vault through the bridge, against a real service whose vault key is a file in a temporary
//! profile, so no keyring is touched. The store holds names and metadata only: no value reaches it.

use std::path::Path;
use std::time::{Duration, Instant};

use massorca_client::{Client, ConnectOptions};
use massorca_desktop::service::{
    ServiceBridge, ServiceHandle, StoreState, StoreUpdate, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::secrets::SecretSet;
use massorca_server::{RunningService, Service, ServiceConfig, VaultOptions};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(30);
/// An obviously fake value: the tests check that it never comes back out of the bridge.
const VALUE: &str = "vault-bridge-test-value-0c1d";

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

async fn start_service(home: &Path) -> RunningService {
    Service::start_with(
        ServiceConfig {
            profile: profile(home),
            server_name: "massorca-test".to_owned(),
            server_version: "0.0.0-test".to_owned(),
        },
        VaultOptions { force_file: true },
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

/// The names the store lists, once the list has arrived.
fn names(state: &StoreState) -> Vec<String> {
    match &state.secrets.list {
        Some(Ok(entries)) => entries.iter().map(|entry| entry.name.clone()).collect(),
        _ => Vec::new(),
    }
}

async fn client_for(home: &Path) -> Client {
    Client::connect(
        &profile(home),
        ConnectOptions::new(PeerInfo {
            name: "massorca-test-client".to_owned(),
            version: "0.0.0-test".to_owned(),
        }),
    )
    .await
    .expect("a client connects")
}

#[tokio::test]
async fn the_store_follows_the_vault_by_name_and_never_holds_a_value() {
    let home = tempfile::tempdir().expect("a temporary home");
    let service = start_service(home.path()).await;
    let (bridge, _handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(&mut updates, &mut state, "the vault status and list", |s| {
        s.connection.is_connected()
            && matches!(s.secrets.status, Some(Ok(_)))
            && matches!(s.secrets.list, Some(Ok(_)))
    })
    .await;
    let status = state
        .secrets
        .status
        .clone()
        .expect("a status is held")
        .expect("the status reads");
    assert_eq!(status.state, "ready");
    // The vault key is made by the first secret, so there is no protection to report before it.
    assert_eq!(status.protection, "none");
    assert_eq!(status.count, 0);
    assert!(names(&state).is_empty(), "an empty vault lists nothing");

    // Another client stores a secret; the bridge learns of it from the event, not from a request of its own.
    let client = client_for(home.path()).await;
    client
        .secret_set(&SecretSet {
            scope: "global".to_owned(),
            name: "BRIDGE_TOKEN".to_owned(),
            value: VALUE.to_owned(),
            replace: false,
        })
        .await
        .expect("the secret is stored");
    until(&mut updates, &mut state, "the new name and count", |s| {
        names(s) == ["BRIDGE_TOKEN"]
            && matches!(&s.secrets.status, Some(Ok(status)) if status.count == 1)
    })
    .await;
    let status = state
        .secrets
        .status
        .clone()
        .expect("a status is held")
        .expect("the status reads");
    assert_eq!(
        status.protection, "file",
        "the key is kept in the profile file"
    );
    let entries = match &state.secrets.list {
        Some(Ok(entries)) => entries.clone(),
        other => panic!("the list reads: {other:?}"),
    };
    assert_eq!(entries[0].scope, "global");
    assert_eq!(entries[0].owner_scope, "global");
    assert!(!entries[0].linked);

    let shown = format!("{state:?}");
    assert!(!shown.contains(VALUE), "the store holds a value: {shown}");

    drop(client);
    drop(bridge);
    stop_service(service).await;
}
