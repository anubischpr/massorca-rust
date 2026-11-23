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
            let entry = entry?;
            // The writers' lock file, and what an interrupted removal or write left behind, are not hosts.
            let name = entry.file_name();
            if name.to_str().is_some_and(|name| {
                name == massorca_profile::LOCK_FILE || massorca_profile::is_leftover(name)
            }) {
                continue;
            }
            let path = entry.path().join(HOST_FILE);
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
    /// others could change. The files are written into a staged folder held open without following links, and the
    /// folder is renamed to the host's name only once all of them are there, so a crash never leaves half a host.
    pub fn add(&self, record: &HostRecord, identity: &TlsIdentity) -> Result<(), ClientError> {
        validate_host_name(&record.name)?;
        let name = record.name.as_str();
        let dir = self.dir.join(name);
        if present(&dir.join(HOST_FILE)).map_err(|error| self.unreadable(name, &error))? {
            return Err(ClientError::HostExists(name.to_owned()));
        }
        massorca_profile::secure_root(&self.profile)?;
        let hosts = open_hosts(&self.dir)?;
        // One writer at a time, across processes: a sweep then never deletes another add's staged folder.
        let _lock = hosts.lock().map_err(busy)?;
        hosts.sweep_leftovers()?;
        // A host folder without a host file is what an interrupted add of an older build left: it never became a host.
        // It is moved aside only when the host file is certainly absent, checked again under the held folder.
        if present(&dir).map_err(|error| self.unreadable(name, &error))? {
            match hosts.remove_child_dir_unless(name, HOST_FILE) {
                Ok(true) => {}
                Ok(false) => return Err(ClientError::HostExists(name.to_owned())),
                Err(massorca_profile::RootAccessError::Io(error)) => {
                    return Err(self.unreadable(name, &error));
                }
                Err(refused) => return Err(refused.into()),
            }
        }
        let (key_pem, cert_pem) = identity.to_pem().map_err(|error| {
            ClientError::Protocol(format!("cannot save the device key: {error}"))
        })?;
        let json = serde_json::to_vec_pretty(record)
            .map_err(|error| ClientError::Protocol(error.to_string()))?;
        let staged = hosts.staged_child()?;
        let written = staged
            .write_file(KEY_FILE, key_pem.as_bytes())
            .and_then(|()| staged.write_file(CERT_FILE, cert_pem.as_bytes()))
            .and_then(|()| staged.write_file(HOST_FILE, &json));
        if let Err(error) = written {
            let _ = hosts.discard(staged);
            return Err(error.into());
        }
        // Publishing never replaces a host folder: when another add published the same name first, this one loses.
        hosts.publish(staged, name).map_err(|error| {
            if present(&dir).unwrap_or(false) {
                ClientError::HostExists(name.to_owned())
            } else {
                error.into()
            }
        })
    }

    /// Forgets a host and deletes its device key. Refuses a profile others could change, and a host folder that is a
    /// link, so nothing outside the profile is deleted. The folder is moved aside through its open handle first, and
    /// only the moved folder is deleted.
    /// When only deleting the moved-aside folder failed, the host is gone and `HostRemoved::leftover` says where its old
    /// files wait for the next host change to delete them.
    pub fn remove(&self, name: &str) -> Result<HostRemoved, ClientError> {
        validate_host_name(name)?;
        if !present(&self.dir.join(name).join(HOST_FILE))
            .map_err(|error| self.unreadable(name, &error))?
        {
            return Err(ClientError::HostNotFound(name.to_owned()));
        }
        massorca_profile::secure_root(&self.profile)?;
        let hosts = open_hosts(&self.dir)?;
        let _lock = hosts.lock().map_err(busy)?;
        hosts.sweep_leftovers()?;
        if !present(&self.dir.join(name).join(HOST_FILE))
            .map_err(|error| self.unreadable(name, &error))?
        {
            return Err(ClientError::HostNotFound(name.to_owned()));
        }
        let leftover = hosts.remove_child_dir(name)?;
        Ok(HostRemoved { leftover })
    }

    /// Whether a host's files are there could not be read: nothing changes, and the message names the folder.
    fn unreadable(&self, name: &str, error: &io::Error) -> ClientError {
        ClientError::HostUnreadable {
            name: name.to_owned(),
            path: self.dir.join(name),
            reason: error.to_string(),
        }
    }
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

/// Whether `path` exists; only "not found" means absent, and any other error is returned.
fn present(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Opens `hosts` without following links; a wait for another writer's hold on it that gave up is `HostsBusy`.
fn open_hosts(dir: &Path) -> Result<massorca_profile::PrivateDir, ClientError> {
    massorca_profile::open_private_dir(dir).map_err(|error| match error {
        massorca_profile::RootAccessError::Io(error) => busy(error),
        refused => refused.into(),
    })
}

/// A lock wait that gave up becomes `HostsBusy`; any other error stays an I/O error.
fn busy(error: io::Error) -> ClientError {
    if error.kind() == io::ErrorKind::TimedOut {
        ClientError::HostsBusy(error.to_string())
    } else {
        error.into()
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
            let made = massorca_process::command("cmd")
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

    fn private_profile(temp: &std::path::Path) -> Profile {
        let profile =
            Profile::resolve_with(Some(temp.join("home")), Some("default".to_owned()), |_| {
                None
            })
            .expect("the profile resolves");
        massorca_profile::secure_root(&profile).expect("a private profile");
        profile
    }

    #[test]
    fn removing_a_host_whose_folder_is_a_link_is_refused_and_the_target_is_untouched() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = private_profile(temp.path());
        let elsewhere = temp.path().join("elsewhere");
        std::fs::create_dir(&elsewhere).expect("a folder");
        std::fs::write(elsewhere.join("host.json"), b"{}").expect("a file it must keep");
        let hosts = profile.dir().join("hosts");
        drop(massorca_profile::open_private_dir(&hosts).expect("hosts"));
        let office_dir = hosts.join("office");
        plant_link(&office_dir, &elsewhere);

        let removed = Hosts::of(&profile).remove("office").map(drop);

        refused_unsafe(removed, &office_dir);
        assert!(
            elsewhere.join("host.json").is_file(),
            "the target is untouched"
        );
    }

    #[test]
    fn adding_a_host_to_a_hosts_folder_others_can_write_is_refused() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = private_profile(temp.path());
        let hosts = profile.dir().join("hosts");
        drop(massorca_profile::open_private_dir(&hosts).expect("hosts"));
        #[cfg(windows)]
        {
            let granted = massorca_process::command("icacls")
                .arg(&hosts)
                .args(["/grant", "*S-1-1-0:(OI)(CI)(M)"])
                .output()
                .expect("icacls runs");
            assert!(
                granted.status.success(),
                "{}",
                String::from_utf8_lossy(&granted.stderr)
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&hosts, std::fs::Permissions::from_mode(0o777))
                .expect("chmod");
        }
        let identity = TlsIdentity::generate("laptop").expect("a device identity");

        match Hosts::of(&profile).add(&office(), &identity) {
            Err(crate::ClientError::ProfileRefused(body)) => {
                assert_eq!(body.code, "profile.writable_by_others", "{body:?}");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert!(!hosts.join("office").exists(), "nothing was written");
    }

    #[test]
    fn a_tombstone_left_by_a_crash_is_not_listed_and_is_swept_on_add_and_remove() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = private_profile(temp.path());
        let identity = TlsIdentity::generate("laptop").expect("a device identity");
        let hosts = Hosts::of(&profile);
        hosts.add(&office(), &identity).expect("a host");
        let dir = profile.dir().join("hosts");
        let crashed = |suffix: &str| {
            let tombstone = dir.join(format!(
                "{}office-{suffix}",
                massorca_profile::TOMBSTONE_PREFIX
            ));
            std::fs::create_dir(&tombstone).expect("a tombstone");
            std::fs::write(tombstone.join("host.json"), b"{\"name\":\"office\"}")
                .expect("a host file");
            std::fs::write(tombstone.join("device.key"), b"old key").expect("a key left behind");
            tombstone
        };

        let first = crashed("first");
        let listed: Vec<String> = hosts
            .list()
            .expect("listed")
            .into_iter()
            .map(|host| host.name)
            .collect();
        assert_eq!(listed, vec!["office".to_owned()]);

        let mut other = office();
        other.name = "home".to_owned();
        hosts.add(&other, &identity).expect("another host");
        assert!(!first.exists(), "add swept the tombstone");

        let second = crashed("second");
        hosts.remove("office").expect("removed");
        assert!(!second.exists(), "remove swept the tombstone");
        let listed: Vec<String> = hosts
            .list()
            .expect("listed")
            .into_iter()
            .map(|host| host.name)
            .collect();
        assert_eq!(listed, vec!["home".to_owned()]);
    }

    #[cfg(windows)]
    #[test]
    fn adding_a_host_to_a_hosts_folder_others_can_read_is_refused() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = private_profile(temp.path());
        let hosts = profile.dir().join("hosts");
        drop(massorca_profile::open_private_dir(&hosts).expect("hosts"));
        let granted = massorca_process::command("icacls")
            .arg(&hosts)
            .args(["/grant", "*S-1-1-0:(OI)(CI)(R)"])
            .output()
            .expect("icacls runs");
        assert!(
            granted.status.success(),
            "{}",
            String::from_utf8_lossy(&granted.stderr)
        );
        let identity = TlsIdentity::generate("laptop").expect("a device identity");

        match Hosts::of(&profile).add(&office(), &identity) {
            Err(crate::ClientError::ProfileRefused(body)) => {
                assert_eq!(body.code, "profile.readable_by_others", "{body:?}");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert!(!hosts.join("office").exists(), "nothing was written");
    }

    #[test]
    fn a_crash_while_adding_leaves_no_host_and_the_next_add_succeeds() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = private_profile(temp.path());
        let identity = TlsIdentity::generate("laptop").expect("a device identity");
        let dir = profile.dir().join("hosts");
        drop(massorca_profile::open_private_dir(&dir).expect("hosts"));
        // This layout: a staged folder that never got its host file.
        let staged = dir.join(".writing-crashed");
        std::fs::create_dir(&staged).expect("a staged folder");
        std::fs::write(staged.join("device.key"), b"old key").expect("a key");
        // An older build's layout: the host folder itself, with a key but no host file.
        let half = dir.join("office");
        std::fs::create_dir(&half).expect("a half-written host");
        std::fs::write(half.join("device.key"), b"old key").expect("a key");
        let hosts = Hosts::of(&profile);
        assert!(
            hosts.list().expect("listed").is_empty(),
            "neither is a host"
        );

        hosts
            .add(&office(), &identity)
            .expect("added over the leftovers");

        assert_eq!(hosts.get("office").expect("the host").name, "office");
        assert_ne!(
            std::fs::read(half.join("device.key")).expect("the new key"),
            b"old key"
        );
        assert!(!staged.exists(), "the staged folder was swept");
        let names: Vec<String> = std::fs::read_dir(&dir)
            .expect("listed")
            .map(|entry| {
                entry
                    .expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        let mut names = names;
        names.sort();
        // The writers' lock file stays; no leftover does.
        assert_eq!(names, vec![".lock".to_owned(), "office".to_owned()]);
    }

    #[test]
    fn windows_reserved_device_names_and_trailing_dots_or_spaces_are_not_host_names() {
        use super::validate_host_name;

        for name in [
            "con",
            "prn",
            "aux",
            "nul",
            "com1",
            "com9",
            "lpt1",
            "lpt9",
            "CON",
            "Nul",
            "aux.txt",
            "nul.tar.gz",
            "office.",
            "office ",
            "office..",
            "com1.",
        ] {
            assert!(validate_host_name(name).is_err(), "{name:?}");
        }
        for name in [
            "console",
            "com0",
            "com10",
            "lpt",
            "auxiliary",
            "null",
            "office",
        ] {
            assert!(validate_host_name(name).is_ok(), "{name:?}");
        }
    }

    /// Makes `host.json` in `office` unreadable to this user without removing anything; `undo` gives access back.
    fn block_host_file(office: &std::path::Path) {
        #[cfg(windows)]
        for (path, right) in [
            (office.to_path_buf(), "(RD)"),
            (office.join("host.json"), "(RA)"),
        ] {
            let denied = massorca_process::command("icacls")
                .arg(&path)
                .args(["/deny", &format!("*S-1-1-0:{right}")])
                .output()
                .expect("icacls runs");
            assert!(
                denied.status.success(),
                "{}",
                String::from_utf8_lossy(&denied.stderr)
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(office, std::fs::Permissions::from_mode(0o000))
                .expect("chmod");
        }
    }

    fn unblock_host_file(office: &std::path::Path) {
        #[cfg(windows)]
        for path in [office.join("host.json"), office.to_path_buf()] {
            let _ = massorca_process::command("icacls")
                .arg(&path)
                .args(["/remove:d", "*S-1-1-0"])
                .output();
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(office, std::fs::Permissions::from_mode(0o700));
        }
    }

    #[test]
    fn a_host_file_that_cannot_be_read_refuses_the_add_and_deletes_nothing() {
        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = private_profile(temp.path());
        let identity = TlsIdentity::generate("laptop").expect("a device identity");
        let hosts = Hosts::of(&profile);
        hosts.add(&office(), &identity).expect("a host");
        let office_dir = profile.dir().join("hosts").join("office");
        let key = std::fs::read(office_dir.join("device.key")).expect("its key");
        block_host_file(&office_dir);
        let blocked = std::fs::symlink_metadata(office_dir.join("host.json"));
        if blocked.as_ref().is_ok_and(|_| true)
            || blocked
                .as_ref()
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
        {
            unblock_host_file(&office_dir);
            panic!("the test could not make the host file unreadable: {blocked:?}");
        }

        let other = TlsIdentity::generate("laptop").expect("another identity");
        let refused = hosts.add(&office(), &other);
        unblock_host_file(&office_dir);

        match refused {
            Err(error @ crate::ClientError::HostUnreadable { .. }) => {
                let message = error.to_string();
                assert!(
                    message.contains(&office_dir.display().to_string()),
                    "{message}"
                );
                assert!(message.contains("remove that folder by hand"), "{message}");
            }
            other => panic!("expected the add to be refused, got {other:?}"),
        }
        assert_eq!(
            std::fs::read(office_dir.join("device.key")).expect("the key"),
            key
        );
        assert!(office_dir.join("host.json").is_file());
    }

    #[test]
    fn two_concurrent_adds_of_one_name_leave_one_whole_host() {
        for _ in 0..5 {
            let temp = tempfile::tempdir().expect("a temporary folder");
            let profile = private_profile(temp.path());
            let hosts = Hosts::of(&profile);
            let identities = [
                TlsIdentity::generate("first").expect("an identity"),
                TlsIdentity::generate("second").expect("an identity"),
            ];
            let start = std::sync::Barrier::new(2);
            let results: Vec<_> = std::thread::scope(|scope| {
                let handles: Vec<_> = identities
                    .iter()
                    .map(|identity| {
                        let (hosts, start) = (&hosts, &start);
                        scope.spawn(move || {
                            start.wait();
                            hosts.add(&office(), identity)
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|handle| handle.join().expect("a thread"))
                    .collect()
            });

            let winners: Vec<usize> = (0..2).filter(|index| results[*index].is_ok()).collect();
            assert_eq!(winners.len(), 1, "{results:?}");
            let loser = &results[1 - winners[0]];
            assert!(
                matches!(loser, Err(crate::ClientError::HostExists(name)) if name == "office"),
                "{loser:?}"
            );
            let (winner_key, _) = identities[winners[0]].to_pem().expect("its key");
            let office_dir = profile.dir().join("hosts").join("office");
            assert_eq!(
                std::fs::read_to_string(office_dir.join("device.key")).expect("the key"),
                winner_key
            );
            assert_eq!(hosts.get("office").expect("the host").name, "office");
        }
    }

    /// Unix only: Windows refuses to rename a folder while a file in it is held open, so it cannot be made to fail
    /// only the delete that follows.
    #[cfg(unix)]
    #[test]
    fn removing_a_host_succeeds_with_a_note_when_only_its_old_files_cannot_be_deleted_yet() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("a temporary folder");
        let profile = private_profile(temp.path());
        let identity = TlsIdentity::generate("laptop").expect("a device identity");
        let hosts = Hosts::of(&profile);
        hosts.add(&office(), &identity).expect("a host");
        let office_dir = profile.dir().join("hosts").join("office");
        // A folder that can still be renamed, but whose entries cannot be removed.
        std::fs::set_permissions(&office_dir, std::fs::Permissions::from_mode(0o500))
            .expect("chmod");

        let removed = hosts.remove("office").expect("the host is removed");

        let leftover = removed.leftover.expect("the old files are reported");
        assert!(
            leftover
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(massorca_profile::TOMBSTONE_PREFIX)),
            "{}",
            leftover.display()
        );
        assert!(hosts.get("office").is_err(), "the host is gone");
        assert!(hosts.list().expect("listed").is_empty());
        std::fs::set_permissions(&leftover, std::fs::Permissions::from_mode(0o700)).expect("chmod");
        hosts
            .add(&office(), &identity)
            .expect("the next add sweeps the old files");
        assert!(!leftover.exists());
    }

    #[test]
    fn a_lock_wait_that_gives_up_is_reported_as_busy() {
        let error = std::io::Error::new(std::io::ErrorKind::TimedOut, "another command is busy");
        assert!(matches!(
            super::busy(error),
            crate::ClientError::HostsBusy(_)
        ));
        let other = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        assert!(matches!(super::busy(other), crate::ClientError::Io(_)));
    }
}
