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
    profile: Profile,
    dir: PathBuf,
}

impl Hosts {
    pub fn of(profile: &Profile) -> Self {
        Self {
            profile: profile.clone(),
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

    /// Checks the profile as a service start does and opens `hosts` without following links, so a pairing is refused
    /// before it uses up an offer when nothing could be saved. Refuses a profile others could change (ADR 0073).
    pub fn ensure_writable(&self) -> Result<(), ClientError> {
        massorca_profile::secure_root(&self.profile)?;
        massorca_profile::open_private_dir(&self.dir)?;
        Ok(())
    }

    /// Stores a new host with its device identity. Refuses to replace an existing host, and to write into a profile
    /// others could change. `hosts` and the host's folder are held open without following links while the files are
    /// written, so neither can be swapped for a link.
    pub fn add(&self, record: &HostRecord, identity: &TlsIdentity) -> Result<(), ClientError> {
        validate_host_name(&record.name)?;
        let dir = self.dir.join(&record.name);
        if dir.join(HOST_FILE).exists() {
            return Err(ClientError::HostExists(record.name.clone()));
        }
        massorca_profile::secure_root(&self.profile)?;
        let _hosts = massorca_profile::open_private_dir(&self.dir)?;
        let _host = massorca_profile::open_private_dir(&dir)?;
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

    /// Forgets a host and deletes its device key. Refuses a profile others could change, and a host folder that is a
    /// link, so nothing outside the profile is deleted.
    pub fn remove(&self, name: &str) -> Result<(), ClientError> {
        validate_host_name(name)?;
        let dir = self.dir.join(name);
        if !dir.join(HOST_FILE).exists() {
            return Err(ClientError::HostNotFound(name.to_owned()));
        }
        massorca_profile::secure_root(&self.profile)?;
        let _hosts = massorca_profile::open_private_dir(&self.dir)?;
        drop(massorca_profile::open_private_dir(&dir)?);
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

#[cfg(test)]
mod tests {
    use massorca_net::TlsIdentity;
    use massorca_profile::Profile;

    use super::{HostRecord, Hosts};

    #[test]
    fn a_new_host_folder_is_private_before_the_device_key_is_written() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = Profile::resolve_with(
            Some(temp.path().join("home")),
            Some("default".to_owned()),
            |_| None,
        )
        .expect("the profile resolves");
        let record = HostRecord {
            name: "office".to_owned(),
            server: "office".to_owned(),
            addresses: vec!["127.0.0.1:1".to_owned()],
            pin: "sha256:pin".to_owned(),
            device_id: "device".to_owned(),
            device_name: "laptop".to_owned(),
            scopes: Vec::new(),
            paired_at_ms: 0,
        };
        let identity = TlsIdentity::generate("laptop").expect("a device identity");

        Hosts::of(&profile)
            .add(&record, &identity)
            .expect("the host is stored");

        let folders = [
            profile.dir().join("hosts"),
            profile.dir().join("hosts").join("office"),
        ];
        for folder in &folders {
            #[cfg(windows)]
            assert!(
                massorca_profile::has_owner_only_dacl(folder).expect("its DACL"),
                "{}: {}",
                folder.display(),
                massorca_profile::dacl_sddl(folder).expect("its DACL")
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(folder)
                    .expect("metadata")
                    .permissions()
                    .mode();
                assert_eq!(mode & 0o777, 0o700, "{}", folder.display());
            }
        }
        assert_eq!(
            massorca_profile::secure_root(&profile).expect("checked"),
            massorca_profile::RootAccess::Private
        );
    }

    fn office() -> HostRecord {
        HostRecord {
            name: "office".to_owned(),
            server: "office".to_owned(),
            addresses: vec!["127.0.0.1:1".to_owned()],
            pin: "sha256:pin".to_owned(),
            device_id: "device".to_owned(),
            device_name: "laptop".to_owned(),
            scopes: Vec::new(),
            paired_at_ms: 0,
        }
    }

    /// A link at `link` to `target`: a junction on Windows, a symlink on Unix.
    fn plant_link(link: &std::path::Path, target: &std::path::Path) {
        #[cfg(windows)]
        {
            let made = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()
                .expect("mklink runs");
            assert!(
                made.status.success(),
                "{}",
                String::from_utf8_lossy(&made.stderr)
            );
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, link).expect("a symlink");
    }

    fn refused_unsafe(result: Result<(), crate::ClientError>, link: &std::path::Path) {
        match result {
            Err(crate::ClientError::ProfileRefused(body)) => {
                assert_eq!(body.code, "profile.unsafe_location", "{body:?}");
                assert_eq!(
                    body.data,
                    Some(serde_json::json!({ "path": link.display().to_string() }))
                );
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_link_planted_at_hosts_is_refused_and_nothing_is_written_through_it() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = Profile::resolve_with(
            Some(temp.path().join("home")),
            Some("default".to_owned()),
            |_| None,
        )
        .expect("the profile resolves");
        massorca_profile::secure_root(&profile).expect("a private profile");
        let elsewhere = temp.path().join("elsewhere");
        std::fs::create_dir(&elsewhere).expect("a folder");
        let hosts = profile.dir().join("hosts");
        plant_link(&hosts, &elsewhere);
        let identity = TlsIdentity::generate("laptop").expect("a device identity");

        refused_unsafe(Hosts::of(&profile).add(&office(), &identity), &hosts);
        refused_unsafe(Hosts::of(&profile).ensure_writable(), &hosts);

        let written: Vec<_> = std::fs::read_dir(&elsewhere).expect("listed").collect();
        assert!(written.is_empty(), "{written:?}");
    }

    #[test]
    fn a_link_planted_at_the_host_folder_is_refused_and_nothing_is_written_through_it() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = Profile::resolve_with(
            Some(temp.path().join("home")),
            Some("default".to_owned()),
            |_| None,
        )
        .expect("the profile resolves");
        massorca_profile::secure_root(&profile).expect("a private profile");
        let elsewhere = temp.path().join("elsewhere");
        std::fs::create_dir(&elsewhere).expect("a folder");
        let hosts = profile.dir().join("hosts");
        massorca_profile::open_private_dir(&hosts).expect("hosts");
        let office_dir = hosts.join("office");
        plant_link(&office_dir, &elsewhere);
        let identity = TlsIdentity::generate("laptop").expect("a device identity");

        refused_unsafe(Hosts::of(&profile).add(&office(), &identity), &office_dir);

        let written: Vec<_> = std::fs::read_dir(&elsewhere).expect("listed").collect();
        assert!(written.is_empty(), "{written:?}");
    }
}
