//! `lock.json`, version 1: what is installed in a profile. Unknown fields are kept on read and write, so a newer
//! service can add fields an older one preserves.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use massorca_ext_package::ids::{is_extension_id, is_kind_id};
use massorca_ext_package::package_path;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::Error;

/// The lockfile format this crate reads and writes.
pub const LOCKFILE_VERSION: u32 = 1;

/// The whole lockfile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lockfile {
    pub version: u32,
    #[serde(default)]
    pub packages: Vec<LockEntry>,
    /// The one-time changes this profile has made, by name, such as `builtin-agents` (decision 0063).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub migrations: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            version: LOCKFILE_VERSION,
            packages: Vec::new(),
            migrations: Vec::new(),
            extra: Map::new(),
        }
    }
}

/// One installed package.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LockEntry {
    pub id: String,
    pub version: String,
    pub source: LockSource,
    pub sha256: String,
    /// The canonical hash of the unpacked files, checked before an installed manifest is trusted. It equals `sha256`
    /// for a folder or catalogue entry; an archive's `sha256` is the archive's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files_sha256: Option<String>,
    /// Relative to the extensions folder, `/`-separated: `pkgs/<id>/<version>-<hash8>`.
    pub path: String,
    pub enabled: bool,
    /// The consent fingerprint the person accepted.
    pub consent: String,
    pub installed_at_ms: u64,
    #[serde(default)]
    pub cli: Vec<LockCli>,
    /// The package's agent kind ids at install, so its kinds can be named even when its files cannot be trusted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agent_kinds: Vec<String>,
    /// Registered at upgrade as a built-in, without a vendor command or consent (decision 0063).
    #[serde(default, skip_serializing_if = "is_false")]
    pub adopted: bool,
    /// The version an update replaced, kept for `extensions.rollback`. Absent until an update replaces the files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<Box<KeptVersion>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Where a package was installed from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockSource {
    pub kind: SourceKind,
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The three tier-1 sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Catalogue,
    Dir,
    File,
}

/// What the install did about one agent adapter's CLI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LockCli {
    pub agent: String,
    pub binary: String,
    pub detected_before_install: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installer: Option<InstallerRun>,
    pub installed_by_package: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The vendor installer that ran: the exact argv, when, and its exit code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstallerRun {
    pub command: Vec<String>,
    pub ran_at_ms: u64,
    pub exit_code: Option<i32>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The package files of one version an update replaced. Rollback swaps them back in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeptVersion {
    pub version: String,
    pub source: LockSource,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files_sha256: Option<String>,
    /// Relative to the extensions folder, `/`-separated, under the package's own `pkgs/<id>/`.
    pub path: String,
    pub consent: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agent_kinds: Vec<String>,
    /// The path of the version that replaced this one. The kept version is a rollback target only while the current
    /// version still has this path: an older binary can carry it forward past a later update.
    #[serde(default)]
    pub replaced_path: String,
    /// Whether the version was registered without consent (decision 0063).
    #[serde(default, skip_serializing_if = "is_false")]
    pub adopted: bool,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl KeptVersion {
    /// The files of `entry`, as the version that replaces it at `replaced_by` keeps them.
    pub(crate) fn of(entry: &LockEntry, replaced_by: &str) -> Self {
        Self {
            version: entry.version.clone(),
            source: entry.source.clone(),
            sha256: entry.sha256.clone(),
            files_sha256: entry.files_sha256.clone(),
            path: entry.path.clone(),
            consent: entry.consent.clone(),
            agent_kinds: entry.agent_kinds.clone(),
            replaced_path: replaced_by.to_owned(),
            adopted: entry.adopted,
            extra: Map::new(),
        }
    }
}

impl Lockfile {
    /// Reads the lockfile; a missing file is an empty lockfile.
    pub fn read(path: &Path) -> Result<Self, Error> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(Error::io("cannot read", path)(error)),
        };
        let lockfile: Self = serde_json::from_slice(&bytes)
            .map_err(|error| Error::InvalidLockfile(error.to_string()))?;
        if lockfile.version != LOCKFILE_VERSION {
            return Err(Error::InvalidLockfile(format!(
                "version {} is not supported, this service reads version {LOCKFILE_VERSION}",
                lockfile.version
            )));
        }
        for entry in &lockfile.packages {
            entry.check()?;
        }
        Ok(lockfile)
    }

    /// Writes the lockfile atomically: a temporary file in the same folder, flushed and synced, then renamed over it.
    pub fn write(&self, path: &Path) -> Result<(), Error> {
        // Refused before anything is written: a lockfile that `read` would refuse must never reach the disk.
        self.packages.iter().try_for_each(LockEntry::check)?;
        let mut bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| Error::InvalidLockfile(error.to_string()))?;
        bytes.push(b'\n');
        write_atomic(path, &bytes)
    }

    pub fn get(&self, id: &str) -> Option<&LockEntry> {
        self.packages.iter().find(|entry| entry.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut LockEntry> {
        self.packages.iter_mut().find(|entry| entry.id == id)
    }

    /// Whether the one-time change `name` has run on this profile.
    pub fn has_migration(&self, name: &str) -> bool {
        self.migrations.iter().any(|done| done == name)
    }

    /// Records the one-time change `name`, once.
    pub fn record_migration(&mut self, name: &str) {
        if !self.has_migration(name) {
            self.migrations.push(name.to_owned());
        }
    }
}

impl LockEntry {
    /// The id and path are used to build paths that removal deletes, so an entry that names anything outside its own
    /// `pkgs/<id>/` folder is refused.
    pub(crate) fn check(&self) -> Result<(), Error> {
        if !is_extension_id(&self.id) {
            return Err(Error::InvalidLockfile(format!(
                "{:?} is not an extension id",
                self.id
            )));
        }
        if let Some(kind) = self.agent_kinds.iter().find(|kind| !is_kind_id(kind)) {
            return Err(Error::InvalidLockfile(format!(
                "{kind:?} in {} is not an agent kind id",
                self.id
            )));
        }
        check_package_path(&self.id, &self.path)?;
        if let Some(kept) = &self.previous {
            check_package_path(&self.id, &kept.path)?;
        }
        Ok(())
    }
}

/// A path is refused unless it names a version folder of its own package, `pkgs/<id>/<version>`.
fn check_package_path(id: &str, path: &str) -> Result<(), Error> {
    let inside = path
        .strip_prefix("pkgs/")
        .and_then(|rest| rest.strip_prefix(id))
        .and_then(|rest| rest.strip_prefix('/'))
        .is_some_and(|version| !version.is_empty() && !version.contains('/'));
    if !inside || package_path::check(path, false).is_err() {
        return Err(Error::InvalidLockfile(format!(
            "the path of {id} is not under pkgs/{id}/"
        )));
    }
    Ok(())
}

/// Writes `bytes` to `path` atomically: a temporary file in the same folder, synced, then renamed over it.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir).map_err(Error::io("cannot create", dir))?;
    let temp = dir.join(format!(
        ".lock.json.{}.{}.tmp",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let written = (|| {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)?;
        sync_dir(dir)
    })();
    written.map_err(|error| {
        let _ = fs::remove_file(&temp);
        Error::io("cannot write", path)(error)
    })
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Makes the rename durable: on Unix the folder entry is synced too. Windows has no folder handle to sync.
pub(crate) fn sync_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    fs::File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_lockfile_is_empty() {
        let dir = tempfile::tempdir().expect("temp dir");
        let lockfile = Lockfile::read(&dir.path().join("lock.json")).expect("read");
        assert_eq!(lockfile, Lockfile::default());
    }

    #[test]
    fn unknown_fields_survive_a_round_trip_at_every_level() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("lock.json");
        let text = serde_json::json!({
            "version": 1, "future": { "a": 1 },
            "packages": [{
                "id": "acme.kit", "version": "1.0.0",
                "source": { "kind": "dir", "ref": "C:/Users/me/kit", "mirror": "x" },
                "sha256": "ab", "path": "pkgs/acme.kit/1.0.0-ab", "enabled": true,
                "consent": "c", "installed_at_ms": 5, "pinned": true,
                "cli": [{ "agent": "kit", "binary": "kit", "detected_before_install": false,
                          "installer": { "command": ["kit-setup"], "ran_at_ms": 6, "exit_code": 0, "pid": 9 },
                          "installed_by_package": true, "note": "n" }]
            }]
        });
        fs::write(&path, text.to_string()).expect("seed");
        let mut lockfile = Lockfile::read(&path).expect("read");
        lockfile.get_mut("acme.kit").expect("entry").enabled = false;
        lockfile.write(&path).expect("write");
        let written: Value =
            serde_json::from_slice(&fs::read(&path).expect("read back")).expect("json");
        let mut expected = text;
        expected["packages"][0]["enabled"] = Value::Bool(false);
        assert_eq!(written, expected);
        let leftovers: Vec<_> = fs::read_dir(dir.path()).expect("list").collect();
        assert_eq!(leftovers.len(), 1, "no temporary file is left behind");
    }

    #[test]
    fn an_entry_naming_a_path_outside_its_package_folder_is_refused() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("lock.json");
        let entry = |id: &str, rel: &str| {
            serde_json::json!({ "version": 1, "packages": [{
                "id": id, "version": "1.0.0", "source": { "kind": "dir", "ref": "x" },
                "sha256": "ab", "path": rel, "enabled": true, "consent": "c", "installed_at_ms": 0
            }] })
        };
        for (id, rel) in [
            ("..", "pkgs/../1.0.0-ab"),
            ("acme.kit", "pkgs/acme.kit/../../outside"),
            ("acme.kit", "pkgs/other.kit/1.0.0-ab"),
            ("acme.kit", "data/acme.kit"),
            ("acme.kit", "pkgs/acme.kit"),
            ("acme.kit", "pkgs/acme.kit/1.0.0-ab/deeper"),
            ("acme.kit", "pkgs/acme.kit/C:"),
        ] {
            fs::write(&path, entry(id, rel).to_string()).expect("seed");
            let error = Lockfile::read(&path).expect_err(rel);
            assert_eq!(
                error.code(),
                crate::error::codes::INVALID_LOCKFILE,
                "{id} {rel}"
            );
        }
        fs::write(
            &path,
            entry("acme.kit", "pkgs/acme.kit/1.0.0-ab").to_string(),
        )
        .expect("seed");
        Lockfile::read(&path).expect("a valid entry reads");
    }

    #[test]
    fn a_kept_version_naming_a_path_outside_its_package_folder_is_refused() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("lock.json");
        let entry = |kept_path: &str| {
            serde_json::json!({ "version": 1, "packages": [{
                "id": "acme.kit", "version": "1.3.0", "source": { "kind": "dir", "ref": "x" },
                "sha256": "ab", "path": "pkgs/acme.kit/1.3.0-ab", "enabled": true, "consent": "c",
                "installed_at_ms": 0,
                "previous": { "version": "1.2.0", "source": { "kind": "dir", "ref": "x" },
                              "sha256": "cd", "path": kept_path, "consent": "c" }
            }] })
        };
        fs::write(&path, entry("pkgs/other.kit/1.2.0-cd").to_string()).expect("seed");
        let error = Lockfile::read(&path).expect_err("a foreign kept path is refused");
        assert_eq!(error.code(), crate::error::codes::INVALID_LOCKFILE);
        fs::write(&path, entry("pkgs/acme.kit/1.2.0-cd").to_string()).expect("seed");
        let lockfile = Lockfile::read(&path).expect("a kept version of its own reads");
        assert_eq!(
            lockfile.packages[0]
                .previous
                .as_ref()
                .map(|kept| kept.version.as_str()),
            Some("1.2.0")
        );
    }

    #[test]
    fn an_entry_read_would_refuse_is_never_written() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("lock.json");
        let entry = serde_json::json!({ "version": 1, "packages": [{
            "id": "con.kit", "version": "1.0.0", "source": { "kind": "dir", "ref": "x" },
            "sha256": "ab", "path": "pkgs/con.kit/1.0.0-ab", "enabled": true, "consent": "c",
            "installed_at_ms": 0
        }] });
        let lockfile: Lockfile = serde_json::from_value(entry).expect("shape");
        let error = lockfile
            .write(&path)
            .expect_err("a device-name publisher is refused");
        assert_eq!(error.code(), crate::error::codes::INVALID_LOCKFILE);
        assert!(!path.exists(), "nothing reached the disk");
    }

    #[test]
    fn another_version_is_refused() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("lock.json");
        fs::write(&path, r#"{"version":2,"packages":[]}"#).expect("seed");
        let error = Lockfile::read(&path).expect_err("version 2 is refused");
        assert_eq!(error.code(), crate::error::codes::INVALID_LOCKFILE);
    }
}
