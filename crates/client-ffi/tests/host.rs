//! The binding driven from Rust as the app drives it (decision 0080, task A2): pairing, connecting, whoami and the
//! protocol gate against a real in-process server, with the hosts kept in a sealed in-memory store.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use massorca_client::{Client, ConnectOptions};
use massorca_client_ffi::{FfiError, HostStore, MassOrca, StorageError, parse_offer};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::devices::{
    self, DeviceRevokeParams, NetworkStatus, PairingCreateParams, PairingCreated,
};
use massorca_server::{ListenSpec, NetworkConfig, RunningService, Service, ServiceConfig};
use tokio::runtime::Runtime;

/// The app's store in memory; sealing flips every byte.
#[derive(Default)]
struct Memory(Mutex<BTreeMap<String, Vec<u8>>>);

impl HostStore for Memory {
    fn read_host(&self, name: String) -> Result<Option<Vec<u8>>, StorageError> {
        Ok(self.0.lock().unwrap().get(&name).cloned())
    }

    fn write_host(&self, name: String, entry: Vec<u8>) -> Result<(), StorageError> {
        self.0.lock().unwrap().insert(name, entry);
        Ok(())
    }

    fn remove_host(&self, name: String) -> Result<bool, StorageError> {
        Ok(self.0.lock().unwrap().remove(&name).is_some())
    }

    fn list_hosts(&self) -> Result<Vec<String>, StorageError> {
        Ok(self.0.lock().unwrap().keys().cloned().collect())
    }

    fn seal(&self, plain: Vec<u8>) -> Result<Vec<u8>, StorageError> {
        Ok(plain.into_iter().map(|byte| !byte).collect())
    }

    fn unseal(&self, sealed: Vec<u8>) -> Result<Vec<u8>, StorageError> {
        Ok(sealed.into_iter().map(|byte| !byte).collect())
    }
}

/// A private home inside a temporary folder; the service refuses one others could reach.
fn temp_home() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let home = temp.path().join("home");
    massorca_profile::create_home(&home).expect("a private home");
    (temp, home)
}

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("server".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

/// The owner's local connection to the server under `home`.
async fn owner(home: &Path) -> Client {
    let peer = PeerInfo {
        name: "massorca-test".to_owned(),
        version: "0.0.0-test".to_owned(),
    };
    Client::connect(&profile(home), ConnectOptions::new(peer))
        .await
        .expect("the owner connects")
}

/// A service listening on an ephemeral loopback port, once its listener is bound.
async fn start(home: &Path) -> RunningService {
    let network = NetworkConfig {
        listen: vec![ListenSpec::Address(SocketAddr::from((
            Ipv4Addr::LOCALHOST,
            0,
        )))],
    };
    let config = ServiceConfig {
        profile: profile(home),
        server_name: "test-server".to_owned(),
        server_version: "0.0.0-test".to_owned(),
    };
    let running = Service::start_with_network(config, network)
        .await
        .expect("the service starts");
    let owner = owner(home).await;
    for _ in 0..200 {
        let status: NetworkStatus = owner.call(devices::NETWORK_STATUS, &()).await.unwrap();
        if status
            .listeners
            .iter()
            .all(|listener| listener.state == "listening")
        {
            return running;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the network listener did not start");
}

async fn stop(running: RunningService) {
    running.shutdown_handle().shutdown();
    tokio::time::timeout(Duration::from_secs(10), running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

async fn offer(home: &Path) -> String {
    let created: PairingCreated = owner(home)
        .await
        .call(devices::PAIRING_CREATE, &PairingCreateParams::default())
        .await
        .expect("an offer");
    created.offer.expose().to_owned()
}

fn app() -> Arc<MassOrca> {
    MassOrca::new(
        Arc::new(Memory::default()),
        "phone".into(),
        "0.0.0-test".into(),
    )
    .expect("the binding starts")
}

#[test]
fn the_phone_pairs_reconnects_and_reads_who_it_is() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().unwrap();
    let running = test.block_on(start(&home));
    let massorca = app();

    let text = test.block_on(offer(&home));
    let summary = parse_offer(text.clone()).unwrap();
    let paired = test
        .block_on(massorca.pair(text.clone(), "office".into()))
        .expect("paired");
    assert_eq!(paired.name, "office");
    assert_eq!(paired.server, summary.server);
    assert_eq!(paired.device_name, "phone");
    assert_eq!(massorca.hosts().unwrap(), vec![paired.clone()]);

    let host = test
        .block_on(massorca.connect("office".into()))
        .expect("connected");
    let me = test.block_on(host.whoami()).expect("whoami");
    assert_eq!(me.device_name.as_deref(), Some("phone"));
    assert!(me.scopes.contains(&"device-admin".to_owned()), "{me:?}");
    let protocol = host.protocol();
    assert_eq!(protocol.server_name, "test-server");
    assert!(protocol.protocol > 0);

    // A second connection with the stored, sealed key.
    drop(host);
    let again = test
        .block_on(massorca.connect("office".into()))
        .expect("reconnected");
    assert_eq!(
        test.block_on(again.whoami()).unwrap().device_id,
        me.device_id
    );

    // The offer worked once; the name is taken.
    let reused = test.block_on(massorca.pair(text, "second".into()));
    assert_eq!(reused.map(|_| ()).unwrap_err().code(), "invalid_offer");
    let fresh = test.block_on(offer(&home));
    let taken = test.block_on(massorca.pair(fresh, "office".into()));
    assert_eq!(taken.map(|_| ()).unwrap_err().code(), "host_exists");

    massorca.forget("office".into()).expect("forgotten");
    assert!(massorca.hosts().unwrap().is_empty());
    let gone = test.block_on(massorca.connect("office".into()));
    assert_eq!(gone.map(|_| ()).unwrap_err().code(), "host_not_found");
    test.block_on(stop(running));
}

#[test]
fn a_revoked_phone_must_pair_again_and_a_stopped_host_is_worth_a_retry() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().unwrap();
    let running = test.block_on(start(&home));
    let massorca = app();
    let text = test.block_on(offer(&home));
    test.block_on(massorca.pair(text, "office".into()))
        .expect("paired");

    test.block_on(async {
        owner(&home)
            .await
            .device_revoke(&DeviceRevokeParams {
                device: "phone".into(),
            })
            .await
            .expect("revoked");
    });
    let refused = test
        .block_on(massorca.connect("office".into()))
        .map(|_| ())
        .unwrap_err();
    assert_eq!(
        refused,
        FfiError::NotPaired {
            message: "office no longer trusts this phone. Pair again.".into()
        }
    );
    assert!(!refused.is_transient());

    test.block_on(stop(running));
    let stopped = test
        .block_on(massorca.connect("office".into()))
        .map(|_| ())
        .unwrap_err();
    assert_eq!(stopped.code(), "unreachable", "{stopped:?}");
    assert!(stopped.is_transient());
}
