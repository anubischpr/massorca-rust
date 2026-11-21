//! The service lifecycle through the client library: autostart, restarts, and starts that race each other.

mod harness;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use assert_cmd::cargo::cargo_bin;
use harness::Sandbox;
use massorca_client::{Autostart, Client, ConnectOptions};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

/// Options that start the `massorca` binary when nothing listens.
fn autostarting(exe: &Path) -> ConnectOptions {
    let mut options = ConnectOptions::new(PeerInfo {
        name: "massorca-test-client".to_owned(),
        version: "0.0.0-test".to_owned(),
    });
    options.autostart = Some(Autostart {
        exe: exe.to_path_buf(),
        args_extra: Vec::new(),
    });
    options
}

fn massorca() -> PathBuf {
    cargo_bin("massorca")
}

#[tokio::test]
async fn autostart_through_the_client_starts_the_service_and_connects() {
    let sandbox = Sandbox::new();
    sandbox.track("default");

    let client = Client::connect(&profile(sandbox.home()), autostarting(&massorca()))
        .await
        .expect("the client starts the service and connects");
    let info = client.service_info().await.expect("service.info answers");
    assert_eq!(info.profile, "default");
    assert_eq!(sandbox.status("default")["running"], true);
}

#[tokio::test]
async fn two_starts_at_once_share_one_service() {
    let sandbox = Sandbox::new();
    sandbox.track("default");
    let (profile, exe) = (profile(sandbox.home()), massorca());

    let (first, second) = tokio::join!(
        Client::connect(&profile, autostarting(&exe)),
        Client::connect(&profile, autostarting(&exe)),
    );
    let first = first.expect("the first client connects");
    let second = second.expect("the second client connects");
    let pids: BTreeSet<u32> = [
        first
            .service_info()
            .await
            .expect("service.info answers")
            .pid,
        second
            .service_info()
            .await
            .expect("service.info answers")
            .pid,
    ]
    .into_iter()
    .collect();
    assert_eq!(pids.len(), 1, "both clients reach the one service that won");
}

#[tokio::test]
async fn restarting_through_the_client_five_times_starts_a_new_service_each_time() {
    let sandbox = Sandbox::new();
    sandbox.track("default");
    let (profile, exe) = (profile(sandbox.home()), massorca());

    let mut pids = BTreeSet::new();
    for _ in 0..5 {
        let client = Client::connect(&profile, autostarting(&exe))
            .await
            .expect("the client starts or reaches the service");
        pids.insert(
            client
                .service_info()
                .await
                .expect("service.info answers")
                .pid,
        );
        client.shutdown().await.expect("the shutdown is answered");
        drop(client);
    }
    assert_eq!(pids.len(), 5, "every round reached a new service");
}
