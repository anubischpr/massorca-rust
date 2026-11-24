//! Helpers shared by the binding's integration tests: a service in this process, a private home, and the owner's
//! connection to it.

use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use massorca_client::{Client, ConnectOptions};
use massorca_client_ffi::{HostStore, MassOrca, StorageError};
use massorca_profile::Profile;
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::devices::{
    self, NetworkStatus, PairingCreateParams, PairingCreated,
};
use massorca_server::{ListenSpec, NetworkConfig, RunningService, Service, ServiceConfig};

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
pub fn temp_home() -> (tempfile::TempDir, PathBuf) {
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
pub async fn owner(home: &Path) -> Client {
    let peer = PeerInfo {
        name: "massorca-test".to_owned(),
        version: "0.0.0-test".to_owned(),
    };
    Client::connect(&profile(home), ConnectOptions::new(peer))
        .await
        .expect("the owner connects")
}

/// A service listening on loopback `port` (0 for an ephemeral one), once its listener is bound.
pub async fn start(home: &Path, port: u16) -> RunningService {
    let network = NetworkConfig {
        listen: vec![ListenSpec::Address(SocketAddr::from((
            Ipv4Addr::LOCALHOST,
            port,
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

/// Stops the service and waits until it has stopped.
pub async fn stop(running: RunningService) {
    running.shutdown_handle().shutdown();
    tokio::time::timeout(Duration::from_secs(10), running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

/// A pairing offer of the host under `home`, made by its owner.
pub async fn offer(home: &Path) -> String {
    let created: PairingCreated = owner(home)
        .await
        .call(devices::PAIRING_CREATE, &PairingCreateParams::default())
        .await
        .expect("an offer");
    created.offer.expose().to_owned()
}

/// The binding over an empty store, as the app starts with no hosts.
pub fn app() -> Arc<MassOrca> {
    MassOrca::new(
        Arc::new(Memory::default()),
        "phone".into(),
        "0.0.0-test".into(),
    )
    .expect("the binding starts")
}
