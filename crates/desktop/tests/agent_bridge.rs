//! The bridge against a real service in a temporary profile, driven headlessly: no window opens. The
//! agent sessions the service lists reach the store on connect, the way workspaces and terminals do.

use std::path::Path;
use std::time::Duration;

use massorca_client::Client;
use massorca_desktop::service::{ServiceBridge, StoreUpdate, connect_options};
use massorca_profile::Profile;
use massorca_server::{RunningService, Service, ServiceConfig};
use tempfile::TempDir;
use tokio::time::timeout;

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

#[tokio::test]
async fn the_agent_sessions_the_service_lists_reach_the_store_on_connect() {
    let home: TempDir = tempfile::tempdir().expect("a temporary home");
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let mut updates = bridge.take_updates().expect("the updates are there");

    let listed = timeout(WAIT, async {
        loop {
            let update = updates.recv().await.expect("the bridge is alive");
            if let StoreUpdate::AgentsResync(sessions) = update {
                return sessions;
            }
        }
    })
    .await
    .expect("the agent sessions arrive after the connect");
    assert!(
        listed.is_empty(),
        "a new profile has no agent sessions: {listed:?}"
    );
    // An empty list also comes from a service that predates agents, so ask the service directly too.
    let client = Client::connect(&profile(home.path()), connect_options(None))
        .await
        .expect("a client connects");
    let direct = client
        .agent_list(None)
        .await
        .expect("the service answers agent.list");
    assert!(direct.sessions.is_empty());

    drop(bridge);
    stop_service(service).await;
}
