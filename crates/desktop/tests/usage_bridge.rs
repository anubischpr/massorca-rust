//! This computer's agent usage through a real service in a temporary profile, driven headlessly (desktop track A, the
//! usage sidebar for ACC-09, ACC-10 and ACC-15): usage is off until its setting is on, turning the setting on reads the
//! usage again, and a refresh of an account that does not exist is refused. No window opens.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_desktop::service::{
    ServiceBridge, ServiceHandle, StoreState, StoreUpdate, connect_options,
};
use massorca_profile::Profile;
use massorca_protocol::methods::config::ConfigSet;
use massorca_protocol::methods::usage::{Collection, SETTING_ENABLED, UsageParams};
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

#[tokio::test]
async fn usage_is_off_until_its_setting_is_on_and_an_unknown_account_is_refused() {
    let home = temp_home();
    let service = start_service(home.path()).await;

    let (bridge, handle, mut updates) = bridge_for(home.path());
    let mut state = StoreState::default();
    until(
        &mut updates,
        &mut state,
        "the service offers usage and answers its state on connect",
        |s| s.connection.is_connected() && s.loaded && s.usage_offered && s.usage.is_some(),
    )
    .await;
    let off = state.usage.clone().expect("usage is read on connect");
    assert_eq!(
        off.collection,
        Collection::Off,
        "usage is off until its setting is on"
    );
    assert!(off.accounts.is_empty(), "an off answer carries no accounts");

    handle
        .config_set(ConfigSet {
            owner: None,
            key: SETTING_ENABLED.to_owned(),
            value: true.into(),
        })
        .await
        .expect("the setting is saved");
    until(
        &mut updates,
        &mut state,
        "the setting turns usage on",
        |s| {
            s.usage
                .as_ref()
                .is_some_and(|usage| usage.collection == Collection::On)
        },
    )
    .await;

    let refused = handle
        .usage_refresh(UsageParams {
            kind: None,
            account: Some("no-such-account".to_owned()),
        })
        .await
        .expect_err("an account that does not exist is refused");
    assert!(
        matches!(&refused, massorca_desktop::service::Refusal::Failed { code, .. } if code == "request.invalid_params"),
        "got {refused:?}"
    );

    drop(bridge);
    stop_service(service).await;
}
