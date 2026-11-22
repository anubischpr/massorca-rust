//! The hosts a client profile paired with (decision 0046): one directory per host under `<profile>/hosts/`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use massorca_net::TlsIdentity;
use massorca_profile::Profile;
use serde::{Deserialize, Serialize};

use crate::error::ClientError;

const HOST_FILE: &str = "host.json";
const KEY_FILE: &str = "device.key";
const CERT_FILE: &str = "device.crt";

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

/// The hosts of one client profile.
#[derive(Debug, Clone)]
pub struct Hosts {
    dir: PathBuf,
}

impl Hosts {
    pub fn of(profile: &Profile) -> Self {
        Self {
            dir: profile.dir().join("hosts"),
        }
    }

    /// Every host, by name. A host whose file cannot be read is skipped with a warning, so one damaged host does not
    /// hide the others.
    pub fn list(&self) -> Result<Vec<HostRecord>, ClientError> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let mut hosts = Vec::new();
        for entry in entries {
            let path = entry?.path().join(HOST_FILE);
            match read_record(&path) {
                Ok(Some(record)) => hosts.push(record),
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(path = %path.display(), %error, "skipping a host file")
                }
            }
        }
        hosts.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(hosts)
    }

    pub fn get(&self, name: &str) -> Result<HostRecord, ClientError> {
        validate_host_name(name)?;
        read_record(&self.dir.join(name).join(HOST_FILE))?
            .ok_or_else(|| ClientError::HostNotFound(name.to_owned()))
    }

    /// The device identity this client presents to `name`.
    pub fn identity(&self, name: &str) -> Result<TlsIdentity, ClientError> {
        validate_host_name(name)?;
        let dir = self.dir.join(name);
        TlsIdentity::load(&dir.join(KEY_FILE), &dir.join(CERT_FILE)).map_err(|error| {
            ClientError::Protocol(format!("cannot read the device key of {name}: {error}"))
        })
    }

    /// Whether a host of that name exists.
    pub fn exists(&self, name: &str) -> bool {
        self.dir.join(name).join(HOST_FILE).is_file()
    }

    /// Stores a new host with its device identity. Refuses to replace an existing host.
    pub fn add(&self, record: &HostRecord, identity: &TlsIdentity) -> Result<(), ClientError> {
        validate_host_name(&record.name)?;
        let dir = self.dir.join(&record.name);
        if dir.join(HOST_FILE).exists() {
            return Err(ClientError::HostExists(record.name.clone()));
        }
        create_private_dir(&self.dir)?;
        create_private_dir(&dir)?;
        identity
            .save(&dir.join(KEY_FILE), &dir.join(CERT_FILE))
            .map_err(|error| {
                ClientError::Protocol(format!("cannot save the device key: {error}"))
            })?;
        let json = serde_json::to_vec_pretty(record)
            .map_err(|error| ClientError::Protocol(error.to_string()))?;
        let temp = dir.join(format!("{HOST_FILE}.tmp"));
        fs::write(&temp, json)?;
        fs::rename(&temp, dir.join(HOST_FILE))?;
        Ok(())
    }

    /// Forgets a host and deletes its device key.
    pub fn remove(&self, name: &str) -> Result<(), ClientError> {
        validate_host_name(name)?;
        let dir = self.dir.join(name);
        if !dir.join(HOST_FILE).exists() {
            return Err(ClientError::HostNotFound(name.to_owned()));
        }
        fs::remove_dir_all(dir)?;
        Ok(())
    }
}

/// Host names follow the profile name rules: `[a-z0-9][a-z0-9_-]{0,31}`; `local` is reserved for the local service.
pub fn validate_host_name(name: &str) -> Result<(), ClientError> {
    let mut chars = name.chars();
    let valid = name.len() <= 32
        && chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && name != "local";
    if valid {
        Ok(())
    } else {
        Err(ClientError::InvalidHostName(name.to_owned()))
    }
}

fn read_record(path: &Path) -> Result<Option<HostRecord>, ClientError> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| ClientError::Protocol(format!("{}: {error}", path.display()))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Creates a directory only the user can enter (0700 on Unix; Windows keeps the profile's per-user access).
fn create_private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
