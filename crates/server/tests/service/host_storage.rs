//! A client that keeps its hosts somewhere other than the profile's files, as the phone does (decision 0080): pairing,
//! connecting and asking who it is go through the `HostStorage` seam alone.

use std::collections::BTreeMap;
use std::sync::Mutex;

use massorca_client::{
    Client, ClientError, HostRecord, HostRemoved, HostStorage, validate_host_name,
};
use massorca_net::TlsIdentity;

use crate::common::{stop, temp_home};
use crate::network::{offer, peer, start_listening};

/// Hosts in memory, with each device identity kept as the PEM text a sealed store would hold.
#[derive(Default)]
struct MemoryHosts(Mutex<BTreeMap<String, (HostRecord, String, String)>>);

impl HostStorage for MemoryHosts {
    fn list(&self) -> Result<Vec<HostRecord>, ClientError> {
        let hosts = self.0.lock().expect("not poisoned");
        Ok(hosts
            .values()
            .map(|(record, _, _)| record.clone())
            .collect())
    }

    fn get(&self, name: &str) -> Result<HostRecord, ClientError> {
        let hosts = self.0.lock().expect("not poisoned");
        hosts
            .get(name)
            .map(|(record, _, _)| record.clone())
            .ok_or_else(|| ClientError::HostNotFound(name.to_owned()))
    }

    fn identity(&self, name: &str) -> Result<TlsIdentity, ClientError> {
        let hosts = self.0.lock().expect("not poisoned");
        let (_, key, cert) = hosts
            .get(name)
            .ok_or_else(|| ClientError::HostNotFound(name.to_owned()))?;
        TlsIdentity::from_pem(key.as_bytes(), cert.as_bytes())
            .map_err(|error| ClientError::Protocol(error.to_string()))
    }

    fn exists(&self, name: &str) -> bool {
        self.0.lock().expect("not poisoned").contains_key(name)
    }

    fn add(&self, record: &HostRecord, identity: &TlsIdentity) -> Result<(), ClientError> {
        validate_host_name(&record.name)?;
        let (key, cert) = identity
            .to_pem()
            .map_err(|error| ClientError::Protocol(error.to_string()))?;
        let mut hosts = self.0.lock().expect("not poisoned");
        if hosts.contains_key(&record.name) {
            return Err(ClientError::HostExists(record.name.clone()));
        }
        hosts.insert(record.name.clone(), (record.clone(), key, cert));
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<HostRemoved, ClientError> {
        match self.0.lock().expect("not poisoned").remove(name) {
            Some(_) => Ok(HostRemoved { leftover: None }),
            None => Err(ClientError::HostNotFound(name.to_owned())),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_client_pairs_and_connects_through_its_own_host_store() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let hosts = MemoryHosts::default();

    let made = offer(home.path(), true, None).await;
    let (paired, record) = Client::pair(&hosts, &made, "office", "phone", &peer())
        .await
        .expect("paired");
    drop(paired);
    assert_eq!(hosts.list().expect("listed"), vec![record.clone()]);

    let again = Client::connect_host(&hosts, "office", &peer())
        .await
        .expect("connected with the stored identity");
    let me = again.device_whoami().await.expect("whoami");
    assert_eq!(me.device.map(|device| device.id), Some(record.device_id));

    let second = Client::pair(&hosts, &made, "office", "phone", &peer()).await;
    assert!(
        matches!(second, Err(ClientError::HostExists(ref name)) if name == "office"),
        "{second:?}"
    );
    hosts.remove("office").expect("forgotten");
    let gone = Client::connect_host(&hosts, "office", &peer()).await;
    assert!(
        matches!(gone, Err(ClientError::HostNotFound(_))),
        "{gone:?}"
    );
    stop(running).await;
}
