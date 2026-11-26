//! The hosted-service calls of the bridge against a real service in a temporary profile, driven headlessly: Gitea
//! connects with a fake token and disconnects, and the rows the Services section draws follow. The vault key goes to
//! a file in the profile. No call lists every service, so no `glab` on this computer is ever run.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_desktop::model::marketplace::{ServiceAction, service_row};
use massorca_desktop::service::{ServiceBridge, StoreState, StoreUpdate, connect_options};
use massorca_profile::Profile;
use massorca_protocol::methods::hosted_services::ServiceConnect;
use massorca_server::{RunningService, Service, ServiceConfig, VaultOptions};
use tempfile::TempDir;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(30);
const TOKEN: &str = "fake-gitea-token-3d9a";

/// A private home below a temporary folder (ADR 0073).
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

/// Waits until the bridge has read the extension lists, which it does once connected.
async fn until_connected(updates: &mut UnboundedReceiver<StoreUpdate>) {
    let mut state = StoreState::default();
    let read = timeout(WAIT, async {
        while state.extensions.is_none() {
            let update = updates.recv().await.expect("the bridge is alive");
            state.apply(update, Instant::now());
        }
    })
    .await;
    assert!(read.is_ok(), "the bridge did not connect");
}

#[tokio::test]
async fn gitea_connects_with_a_fake_token_and_disconnects_through_the_bridge() {
    let home = temp_home();
    let running = start_service(&home.path).await;
    let mut bridge = ServiceBridge::start(profile(&home.path), connect_options(None))
        .expect("the bridge starts");
    let mut updates = bridge.take_updates().expect("the updates are there");
    let handle = bridge.handle();
    until_connected(&mut updates).await;

    let params = ServiceConnect {
        token: TOKEN.to_owned(),
        base_url: Some("https://gitea.example.com".to_owned()),
        ..base_service_connect()
    };
    let connected = handle
        .connect_hosted_service(None, params)
        .await
        .expect("gitea connects");
    let row = service_row(&connected);
    assert_eq!(
        (
            row.state,
            row.address.as_deref(),
            row.action,
            row.can_disconnect
        ),
        (
            "Connected",
            Some("https://gitea.example.com"),
            ServiceAction::Replace,
            true
        )
    );
    assert!(!format!("{connected:?}").contains(TOKEN));

    let gone = handle
        .disconnect_hosted_service(None, "gitea".to_owned())
        .await
        .expect("gitea disconnects");
    let row = service_row(&gone);
    assert_eq!(
        (row.state, row.action),
        ("Not connected", ServiceAction::Connect)
    );

    let refused = handle
        .disconnect_hosted_service(None, "linear".to_owned())
        .await
        .expect_err("linear is status-only");
    assert_eq!(refused.code(), Some("marketplace.service_unavailable"));
    drop(bridge);
    running.shutdown_handle().shutdown();
    timeout(WAIT, running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

/// Every `ServiceConnect` field at its usual value; tests override the fields they vary.
fn base_service_connect() -> massorca_protocol::methods::hosted_services::ServiceConnect {
    massorca_protocol::methods::hosted_services::ServiceConnect {
        service: "gitea".to_owned(),
        token: String::new(),
        base_url: None,
        email: None,
    }
}
