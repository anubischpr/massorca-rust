//! Where the phone keeps its hosts (decision 0080, section 3). The app implements `HostStore` in Kotlin: entries in
//! app-private storage, and `seal`/`unseal` with a Keystore key that never leaves the Keystore. `SealedHosts` turns it
//! into the client's `HostStorage`, so the device key is only ever stored sealed and is plain only in memory.

use std::fmt;
use std::io;
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use massorca_client::{ClientError, HostRecord, HostRemoved, HostStorage, validate_host_name};
use massorca_net::TlsIdentity;
use serde::{Deserialize, Serialize};

/// A failure of the app's store, with a sentence for the log.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum StorageError {
    #[error("{message}")]
    Failed { message: String },
}

impl From<uniffi::UnexpectedUniFFICallbackError> for StorageError {
    fn from(error: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::Failed {
            message: error.reason,
        }
    }
}

/// The app's store: one opaque entry per host name, and a key that seals and unseals bytes.
#[uniffi::export(with_foreign)]
pub trait HostStore: Send + Sync {
    /// The entry of `name`, or `None` when there is none.
    fn read_host(&self, name: String) -> Result<Option<Vec<u8>>, StorageError>;
    /// Writes the entry of `name`, replacing it whole.
    fn write_host(&self, name: String, entry: Vec<u8>) -> Result<(), StorageError>;
    /// Deletes the entry of `name`; whether there was one.
    fn remove_host(&self, name: String) -> Result<bool, StorageError>;
    /// The names that have an entry.
    fn list_hosts(&self) -> Result<Vec<String>, StorageError>;
    /// Encrypts bytes with the device's Keystore key.
    fn seal(&self, plain: Vec<u8>) -> Result<Vec<u8>, StorageError>;
    /// Decrypts what `seal` returned; fails for anything else.
    fn unseal(&self, sealed: Vec<u8>) -> Result<Vec<u8>, StorageError>;
}

/// One stored host: its record, its certificate, and its private key sealed.
#[derive(Serialize, Deserialize)]
struct Entry {
    record: HostRecord,
    certificate: String,
    sealed_key: String,
}

/// The client's `HostStorage` over the app's `HostStore`.
#[derive(Clone)]
pub struct SealedHosts(Arc<dyn HostStore>);

impl fmt::Debug for SealedHosts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SealedHosts")
    }
}

impl SealedHosts {
    pub fn new(store: Arc<dyn HostStore>) -> Self {
        Self(store)
    }

    fn entry(&self, name: &str) -> Result<Option<Entry>, ClientError> {
        validate_host_name(name)?;
        let Some(bytes) = self.0.read_host(name.to_owned()).map_err(failed)? else {
            return Ok(None);
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| failed_with(format!("the entry of {name} is damaged: {error}")))
    }
}

impl HostStorage for SealedHosts {
    fn list(&self) -> Result<Vec<HostRecord>, ClientError> {
        let mut names = self.0.list_hosts().map_err(failed)?;
        names.sort();
        let mut hosts = Vec::new();
        for name in names {
            if let Some(entry) = self.entry(&name)? {
                hosts.push(entry.record);
            }
        }
        Ok(hosts)
    }

    fn get(&self, name: &str) -> Result<HostRecord, ClientError> {
        self.entry(name)?
            .map(|entry| entry.record)
            .ok_or_else(|| ClientError::HostNotFound(name.to_owned()))
    }

    fn identity(&self, name: &str) -> Result<TlsIdentity, ClientError> {
        let entry = self
            .entry(name)?
            .ok_or_else(|| ClientError::HostNotFound(name.to_owned()))?;
        let sealed = STANDARD.decode(entry.sealed_key).map_err(|error| {
            failed_with(format!("the sealed key of {name} is damaged: {error}"))
        })?;
        let key = self.0.unseal(sealed).map_err(failed)?;
        TlsIdentity::from_pem(&key, entry.certificate.as_bytes())
            .map_err(|error| failed_with(format!("the device key of {name} is unusable: {error}")))
    }

    fn exists(&self, name: &str) -> bool {
        matches!(self.entry(name), Ok(Some(_)))
    }

    fn add(&self, record: &HostRecord, identity: &TlsIdentity) -> Result<(), ClientError> {
        let name = record.name.as_str();
        if self.entry(name)?.is_some() {
            return Err(ClientError::HostExists(name.to_owned()));
        }
        let (key, certificate) = identity
            .to_pem()
            .map_err(|error| failed_with(format!("cannot save the device key: {error}")))?;
        let sealed = self.0.seal(key.into_bytes()).map_err(failed)?;
        let entry = Entry {
            record: record.clone(),
            certificate,
            sealed_key: STANDARD.encode(sealed),
        };
        let bytes = serde_json::to_vec(&entry).map_err(|error| failed_with(error.to_string()))?;
        self.0.write_host(name.to_owned(), bytes).map_err(failed)
    }

    fn remove(&self, name: &str) -> Result<HostRemoved, ClientError> {
        validate_host_name(name)?;
        if self.0.remove_host(name.to_owned()).map_err(failed)? {
            Ok(HostRemoved { leftover: None })
        } else {
            Err(ClientError::HostNotFound(name.to_owned()))
        }
    }
}

/// A store failure, as the client reports one; it becomes `FfiError::Storage`.
fn failed(error: StorageError) -> ClientError {
    failed_with(error.to_string())
}

fn failed_with(message: String) -> ClientError {
    ClientError::Io(io::Error::other(message))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use massorca_client::{ClientError, HostRecord, HostStorage};
    use massorca_net::TlsIdentity;

    use super::{HostStore, SealedHosts, StorageError};
    use crate::error::FfiError;

    /// Entries in memory; sealing flips every byte, so a sealed key never reads as PEM.
    #[derive(Default)]
    struct Memory {
        entries: Mutex<BTreeMap<String, Vec<u8>>>,
        broken_key: bool,
    }

    impl HostStore for Memory {
        fn read_host(&self, name: String) -> Result<Option<Vec<u8>>, StorageError> {
            Ok(self.entries.lock().unwrap().get(&name).cloned())
        }

        fn write_host(&self, name: String, entry: Vec<u8>) -> Result<(), StorageError> {
            self.entries.lock().unwrap().insert(name, entry);
            Ok(())
        }

        fn remove_host(&self, name: String) -> Result<bool, StorageError> {
            Ok(self.entries.lock().unwrap().remove(&name).is_some())
        }

        fn list_hosts(&self) -> Result<Vec<String>, StorageError> {
            Ok(self.entries.lock().unwrap().keys().cloned().collect())
        }

        fn seal(&self, plain: Vec<u8>) -> Result<Vec<u8>, StorageError> {
            Ok(plain.into_iter().map(|byte| !byte).collect())
        }

        fn unseal(&self, sealed: Vec<u8>) -> Result<Vec<u8>, StorageError> {
            if self.broken_key {
                return Err(StorageError::Failed {
                    message: "the Keystore key is gone".into(),
                });
            }
            Ok(sealed.into_iter().map(|byte| !byte).collect())
        }
    }

    fn record(name: &str) -> HostRecord {
        HostRecord {
            name: name.to_owned(),
            server: "server".to_owned(),
            addresses: vec!["100.64.0.10:7347".to_owned()],
            pin: "sha256:placeholder".to_owned(),
            device_id: "device-1".to_owned(),
            device_name: "phone".to_owned(),
            scopes: vec!["read".to_owned()],
            paired_at_ms: 1,
        }
    }

    #[test]
    fn a_host_round_trips_with_its_key_stored_only_sealed() {
        let memory = Arc::new(Memory::default());
        let hosts = SealedHosts::new(Arc::clone(&memory) as Arc<dyn HostStore>);
        let identity = TlsIdentity::generate("phone").unwrap();
        hosts.add(&record("vps"), &identity).unwrap();

        let stored = memory.read_host("vps".into()).unwrap().unwrap();
        let text = String::from_utf8_lossy(&stored);
        assert!(!text.contains("PRIVATE KEY"), "the key is stored sealed");
        assert_eq!(hosts.list().unwrap(), vec![record("vps")]);
        assert_eq!(hosts.get("vps").unwrap(), record("vps"));
        assert_eq!(hosts.identity("vps").unwrap().pin(), identity.pin());
        assert!(hosts.exists("vps"));

        let again = hosts.add(&record("vps"), &identity);
        assert!(
            matches!(again, Err(ClientError::HostExists(_))),
            "{again:?}"
        );
        hosts.remove("vps").unwrap();
        assert!(!hosts.exists("vps"));
        assert!(matches!(
            hosts.remove("vps"),
            Err(ClientError::HostNotFound(_))
        ));
    }

    #[test]
    fn a_key_that_cannot_be_unsealed_is_a_storage_error() {
        let identity = TlsIdentity::generate("phone").unwrap();
        let sealing = Arc::new(Memory::default());
        SealedHosts::new(Arc::clone(&sealing) as Arc<dyn HostStore>)
            .add(&record("vps"), &identity)
            .unwrap();
        let broken = Memory {
            entries: Mutex::new(sealing.entries.lock().unwrap().clone()),
            broken_key: true,
        };
        let hosts = SealedHosts::new(Arc::new(broken));
        let error = hosts.identity("vps").expect_err("refused");
        let mapped = FfiError::from_client(error, "vps");
        assert_eq!(mapped.code(), "storage", "{mapped:?}");
        assert!(
            mapped.to_string().contains("the Keystore key is gone"),
            "{mapped}"
        );
    }
}
