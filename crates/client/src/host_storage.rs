//! Where a client keeps the hosts it paired with and their device keys (decision 0080, section 3). The desktop and the
//! CLI use the profile's file store, `Hosts`; the phone supplies its own, backed by the Android Keystore.

use std::path::PathBuf;

use massorca_net::TlsIdentity;
use serde::{Deserialize, Serialize};

use crate::error::ClientError;

/// The paired hosts and the device identity presented to each.
pub trait HostStorage: Send + Sync {
    /// Every host, by name.
    fn list(&self) -> Result<Vec<HostRecord>, ClientError>;
    /// The host `name`, or `HostNotFound`.
    fn get(&self, name: &str) -> Result<HostRecord, ClientError>;
    /// The device identity this client presents to `name`.
    fn identity(&self, name: &str) -> Result<TlsIdentity, ClientError>;
    /// Whether a host of that name exists.
    fn exists(&self, name: &str) -> bool;
    /// Refuses before a pairing uses up its offer when nothing could be saved. A store that can always write keeps the
    /// default.
    fn ensure_writable(&self) -> Result<(), ClientError> {
        Ok(())
    }
    /// Stores a new host with its device identity; never replaces one (`HostExists`).
    fn add(&self, record: &HostRecord, identity: &TlsIdentity) -> Result<(), ClientError>;
    /// Forgets a host and deletes its device key.
    fn remove(&self, name: &str) -> Result<HostRemoved, ClientError>;
}

/// What the client remembers about one paired host. Holds no secret; the device key is a separate file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostRecord {
    pub name: String,
    /// The server's own name, from the offer.
    pub server: String,
    /// Addresses to try in order, `host:port`.
    pub addresses: Vec<String>,
    /// `sha256:<base64url>` of the server's public key.
    pub pin: String,
    /// The id the server gave this device.
    pub device_id: String,
    /// The name this device has on the server.
    pub device_name: String,
    pub scopes: Vec<String>,
    pub paired_at_ms: i64,
}

/// What `Hosts::remove` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRemoved {
    /// Where the host's old files wait when they could not be deleted at once; the next add or remove deletes them.
    pub leftover: Option<PathBuf>,
}

/// Host names follow the profile name rules: `[a-z0-9][a-z0-9_-]{0,31}`; `local` is reserved for the local service.
pub fn validate_host_name(name: &str) -> Result<(), ClientError> {
    let mut chars = name.chars();
    let valid = name.len() <= 32
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && name != "local"
        && !is_windows_reserved(name);
    if valid {
        Ok(())
    } else {
        Err(ClientError::InvalidHostName(name.to_owned()))
    }
}

/// Whether Windows reserves `name` for a device, so no file or folder can have it: CON, PRN, AUX, NUL, COM1 to COM9 and
/// LPT1 to LPT9, in any case, alone or before an extension. Trailing dots and spaces cannot appear in a host name.
fn is_windows_reserved(name: &str) -> bool {
    let base = name.split('.').next().unwrap_or(name).to_ascii_lowercase();
    match base.as_bytes() {
        b"con" | b"prn" | b"aux" | b"nul" => true,
        [b'c', b'o', b'm', digit] | [b'l', b'p', b't', digit] => (b'1'..=b'9').contains(digit),
        _ => false,
    }
}
