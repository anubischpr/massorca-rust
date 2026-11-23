//! The extension service of one profile: review, install, enable, disable and remove. Every change is serialised by
//! one lock, and `lock.json` is committed last, so a failed change leaves nothing registered.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use massorca_ext_package::manifest::agent::{CliOs, is_vendor_program};
use massorca_ext_package::manifest::{self, Manifest};
use massorca_profile::Profile;

use crate::catalogue;
use crate::error::Error;
use crate::hooks::Hooks;
use crate::host::{CommandRunner, Exit, INSTALLER_TIMEOUT, ProgramResolver, current_os};
use crate::layout::Layout;
use crate::lockfile::{InstallerRun, LockCli, LockEntry, LockSource, Lockfile};
use crate::review::{self, CliPlan, PlanState, Review};
use crate::source::{self, Files, LoadedPackage, Source};

/// The extension service of one profile.
pub struct Extensions {
    pub(crate) layout: Layout,
    pub(crate) resolver: Arc<dyn ProgramResolver>,
    pub(crate) runner: Arc<dyn CommandRunner>,
    pub(crate) os: Option<CliOs>,
    pub(crate) hooks: Arc<Hooks>,
    changes: Mutex<()>,
    /// The packages whose install or removal is running, with the agent kinds an install claims.
    installing: Arc<Mutex<BTreeMap<String, Vec<String>>>>,
}

impl fmt::Debug for Extensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Extensions")
            .field("layout", &self.layout)
            .field("os", &self.os)
            .field("hooks", &self.hooks)
            .finish_non_exhaustive()
    }
}

impl Extensions {
    pub fn new(
        profile: &Profile,
        resolver: Arc<dyn ProgramResolver>,
        runner: Arc<dyn CommandRunner>,
    ) -> Self {
        Self {
            layout: Layout::new(profile),
            resolver,
            runner,
            os: current_os(),
            hooks: Arc::default(),
            changes: Mutex::new(()),
            installing: Arc::default(),
        }
    }

    /// Plans installers for `os` instead of the host's OS. `None` is a host no package targets.
    pub fn with_os(mut self, os: Option<CliOs>) -> Self {
        self.os = os;
        self
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Holds the change lock. A panic in an earlier change leaves the files consistent, so poisoning is ignored.
    pub(crate) fn lock_changes(&self) -> MutexGuard<'_, ()> {
        self.changes.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn read_lockfile(&self) -> Result<Lockfile, Error> {
        Lockfile::read(&self.layout.lock_path())
    }

    /// The review of a package, without changing anything. An archive is hashed as read.
    pub fn inspect(&self, source: &Source) -> Result<Review, Error> {
        let package = source::load(source, None)?;
        Ok(review::review(&package, &*self.resolver, self.os))
    }

    /// Installs a package. `consent` is the fingerprint of the accepted review; `run_installer` accepts running a
    /// vendor installer, separately; `sha256` pins the package and is required for an archive.
    ///
    /// The change lock is held to check and unpack, and again to commit. The vendor command runs between the two, so
    /// other packages can be changed meanwhile; this package is busy (extension.busy) for that time.
    pub fn install(
        &self,
        source: &Source,
        consent: Option<&str>,
        run_installer: bool,
        sha256: Option<&str>,
    ) -> Result<LockEntry, Error> {
        let staged = self.stage(source, consent, run_installer, sha256)?;
        let ran = self.run_installers(&staged.manifest, &staged.plan);
        self.commit(staged, ran)
    }

    /// Checks the install and unpacks it, under the change lock. The returned mark keeps the package busy until the
    /// install is committed or abandoned.
    fn stage(
        &self,
        source: &Source,
        consent: Option<&str>,
        run_installer: bool,
        sha256: Option<&str>,
    ) -> Result<Staged, Error> {
        let _changes = self.lock_changes();
        if matches!(source, Source::File(_)) && sha256.is_none() {
            return Err(Error::HashRequired);
        }
        let package = source::load(source, sha256)?;
        let manifest = &package.manifest;
        let lockfile = self.read_lockfile()?;
        let builtin = matches!(source, Source::Catalogue(_));
        displaced(&lockfile, &manifest.id, builtin)?;
        self.check_kind_conflicts(&lockfile, manifest, builtin)?;
        let kinds = agent_kind_ids(manifest);
        self.refuse_if_busy(&manifest.id)?;
        self.check_installing_kinds(&manifest.id, &kinds)?;

        let plan = review::plan(manifest, &*self.resolver, self.os);
        let expected = review::fingerprint(manifest, &package.sha256, &plan, self.os);
        match consent {
            None => return Err(Error::ConsentRequired),
            Some(consent) if consent != expected => return Err(Error::ConsentMismatch),
            Some(_) => {}
        }
        let runs_installer = plan
            .iter()
            .any(|entry| matches!(entry.state, PlanState::WillRun { .. }));
        if runs_installer && !run_installer {
            return Err(Error::InstallerConsentRequired);
        }

        let busy = self.mark_busy(&manifest.id, kinds)?;
        self.unpack_staged(package, busy, plan, expected, builtin)
    }

    /// Unpacks a checked package and checks the folder against the reviewed hash. Callers hold the change lock.
    fn unpack_staged(
        &self,
        package: LoadedPackage,
        busy: Busy,
        plan: Vec<CliPlan>,
        consent: String,
        builtin: bool,
    ) -> Result<Staged, Error> {
        let version = package.manifest.version.to_string();
        let rel = Layout::package_rel_path(&package.manifest.id, &version, &package.sha256);
        let dir = self.layout.resolve(&rel);
        let files_sha256 = source::canonical_sha256(&package.files);
        self.unpack(&package.manifest.id, &package.files, &dir)?;
        if let Err(error) = check_unpacked(&package.manifest.id, &dir, &files_sha256) {
            self.remove_unpacked(&package.manifest.id, &dir);
            return Err(error);
        }
        Ok(Staged {
            _busy: busy,
            manifest: package.manifest,
            plan,
            version,
            rel,
            dir,
            files_sha256,
            source: package.source,
            sha256: package.sha256,
            consent,
            builtin,
        })
    }

    /// Records the install under the change lock, after its vendor command ran. Whatever fails here removes the
    /// unpacked folder, so nothing is registered.
    fn commit(&self, staged: Staged, ran: Result<Vec<LockCli>, Error>) -> Result<LockEntry, Error> {
        let _changes = self.lock_changes();
        let committed = ran.and_then(|cli| {
            let mut lockfile = self.read_lockfile()?;
            let displaced = displaced(&lockfile, &staged.manifest.id, staged.builtin)?;
            self.check_kind_conflicts(&lockfile, &staged.manifest, staged.builtin)?;
            let entry = staged.entry(cli);
            lockfile.packages.retain(|package| package.id != entry.id);
            lockfile.packages.push(entry.clone());
            lockfile.write(&self.layout.lock_path())?;
            Ok((entry, displaced))
        });
        match committed {
            Ok((entry, displaced)) => {
                if let Some(old) = displaced
                    && old.path != entry.path
                {
                    // The displaced entry left the lockfile with this install, so its folder is only litter now.
                    let _ = fs::remove_dir_all(self.layout.resolve(&old.path));
                }
                Ok(entry)
            }
            Err(error) => {
                self.remove_unpacked(&staged.manifest.id, &staged.dir);
                Err(error)
            }
        }
    }

    /// Refuses while an install or a removal of `id` is running.
    pub(crate) fn refuse_if_busy(&self, id: &str) -> Result<(), Error> {
        let installing = self
            .installing
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if installing.contains_key(id) {
            return Err(Error::Busy(id.to_owned()));
        }
        Ok(())
    }

    /// Marks `id` busy until the returned mark drops. Callers hold the change lock.
    pub(crate) fn mark_busy(&self, id: &str, kinds: Vec<String>) -> Result<Busy, Error> {
        let mut installing = self
            .installing
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if installing.contains_key(id) {
            return Err(Error::Busy(id.to_owned()));
        }
        installing.insert(id.to_owned(), kinds);
        Ok(Busy {
            installing: Arc::clone(&self.installing),
            id: id.to_owned(),
        })
    }

    /// An agent kind that another package is installing right now is taken, as an installed one would be.
    fn check_installing_kinds(&self, id: &str, kinds: &[String]) -> Result<(), Error> {
        let installing = self
            .installing
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        for (other, other_kinds) in installing.iter().filter(|(other, _)| other.as_str() != id) {
            if let Some(kind) = kinds.iter().find(|kind| other_kinds.contains(kind)) {
                return Err(Error::KindConflict {
                    kind: kind.clone(),
                    installed_by: other.clone(),
                });
            }
        }
        Ok(())
    }

    /// The manifest of an installed package, read from its unpacked folder.
    pub(crate) fn installed_manifest(&self, entry: &LockEntry) -> Result<Manifest, Error> {
        installed_manifest(&self.layout, entry)
    }

    fn check_kind_conflicts(
        &self,
        lockfile: &Lockfile,
        manifest: &Manifest,
        builtin: bool,
    ) -> Result<(), Error> {
        for entry in &lockfile.packages {
            // A built-in is checked against the built-ins only: no third-party entry can block it.
            if builtin && !catalogue::is_builtin_install(entry) {
                continue;
            }
            // The ids recorded at install, so a package whose files changed never blocks installing another one.
            let installed: Vec<String> = if entry.agent_kinds.is_empty() {
                self.installed_manifest(entry)?
                    .contributes
                    .agent_adapters
                    .into_iter()
                    .map(|adapter| adapter.id)
                    .collect()
            } else {
                entry.agent_kinds.clone()
            };
            for adapter in &manifest.contributes.agent_adapters {
                if installed.contains(&adapter.id) {
                    return Err(Error::KindConflict {
                        kind: adapter.id.clone(),
                        installed_by: entry.id.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Writes the files to a staging folder, then renames it into place.
    fn unpack(&self, id: &str, files: &Files, dir: &Path) -> Result<(), Error> {
        let staging = self.layout.package_root(id).join(format!(
            ".staging-{}-{}",
            std::process::id(),
            STAGING_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let written = (|| {
            for (path, bytes) in files {
                let target = path
                    .split('/')
                    .fold(staging.clone(), |target, segment| target.join(segment));
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(Error::io("cannot create", parent))?;
                }
                // A name that collides with one already written (an alias) fails the install.
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&target)
                    .and_then(|mut file| file.write_all(bytes))
                    .map_err(Error::io("cannot write", &target))?;
            }
            if dir.exists() {
                // A leftover from an install that never reached the lockfile.
                fs::remove_dir_all(dir).map_err(Error::io("cannot replace", dir))?;
            }
            fs::rename(&staging, dir).map_err(Error::io("cannot move into place", dir))
        })();
        if written.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        written
    }

    /// Deletes an unpacked package after a failed install, and its `pkgs/<id>/` folder when nothing else is in it.
    fn remove_unpacked(&self, id: &str, dir: &Path) {
        let _ = fs::remove_dir_all(dir);
        let _ = fs::remove_dir(self.layout.package_root(id));
    }

    /// Runs the planned installers and their verify commands, and records what was done for each CLI.
    fn run_installers(&self, manifest: &Manifest, plan: &[CliPlan]) -> Result<Vec<LockCli>, Error> {
        let mut out = Vec::new();
        for (adapter, entry) in manifest.contributes.agent_adapters.iter().zip(plan) {
            let Some(cli) = &adapter.cli else {
                continue;
            };
            let mut record = LockCli {
                agent: adapter.id.clone(),
                binary: cli.binary.clone(),
                detected_before_install: false,
                installer: None,
                installed_by_package: false,
                extra: Default::default(),
            };
            match &entry.state {
                PlanState::Detected { .. } => record.detected_before_install = true,
                PlanState::Unsupported => {}
                PlanState::WillRun { command } => {
                    if !is_vendor_program(command) {
                        return Err(Error::InstallerFailed {
                            agent: adapter.id.clone(),
                            message: "its program is not on the vendor allowlist".to_owned(),
                        });
                    }
                    let data = self.layout.data_dir(&manifest.id);
                    fs::create_dir_all(&data).map_err(Error::io("cannot create", &data))?;
                    let log = self.layout.install_log(&manifest.id);
                    let ran_at_ms = now_ms();
                    let failed = |message: String| Error::InstallerFailed {
                        agent: adapter.id.clone(),
                        message,
                    };
                    let exit = self
                        .runner
                        .run(command, &log, INSTALLER_TIMEOUT)
                        .map_err(|error| failed(error.to_string()))?;
                    if !exit.success() {
                        return Err(failed(describe(exit)));
                    }
                    let unverified = |message: String| {
                        note_remaining_cli(&log, command);
                        Error::VerifyFailed {
                            agent: adapter.id.clone(),
                            message,
                            installer: Some(command.clone()),
                        }
                    };
                    // Without a verify command the install succeeds when the binary resolves.
                    if let Some(verify) = &cli.verify {
                        let verified = self
                            .runner
                            .run(verify, &log, INSTALLER_TIMEOUT)
                            .map_err(|error| unverified(error.to_string()))?;
                        if !verified.success() {
                            return Err(unverified(describe(verified)));
                        }
                    }
                    if self.resolver.resolve(&cli.binary).is_none() {
                        return Err(unverified(format!("`{}` does not resolve", cli.binary)));
                    }
                    record.installer = Some(InstallerRun {
                        command: command.clone(),
                        ran_at_ms,
                        exit_code: exit.code,
                        extra: Default::default(),
                    });
                    record.installed_by_package = true;
                }
            }
            out.push(record);
        }
        Ok(out)
    }
}

/// An install checked and unpacked, waiting for its vendor command. `_busy` keeps the package marked until it is
/// committed or abandoned.
struct Staged {
    _busy: Busy,
    manifest: Manifest,
    plan: Vec<CliPlan>,
    version: String,
    rel: String,
    dir: PathBuf,
    files_sha256: String,
    source: LockSource,
    sha256: String,
    consent: String,
    builtin: bool,
}

impl Staged {
    /// The lock entry this install records, with what its vendor commands did for each CLI.
    fn entry(&self, cli: Vec<LockCli>) -> LockEntry {
        LockEntry {
            id: self.manifest.id.clone(),
            version: self.version.clone(),
            source: self.source.clone(),
            sha256: self.sha256.clone(),
            files_sha256: Some(self.files_sha256.clone()),
            path: self.rel.clone(),
            enabled: true,
            consent: self.consent.clone(),
            installed_at_ms: now_ms(),
            cli,
            agent_kinds: agent_kind_ids(&self.manifest),
            extra: Default::default(),
        }
    }
}

/// Marks one package busy while its install or removal runs without the change lock. Dropping it clears the mark.
pub(crate) struct Busy {
    installing: Arc<Mutex<BTreeMap<String, Vec<String>>>>,
    id: String,
}

impl Drop for Busy {
    fn drop(&mut self) {
        self.installing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.id);
    }
}

/// The agent kind ids a package declares, as its lock entry records them.
fn agent_kind_ids(manifest: &Manifest) -> Vec<String> {
    manifest
        .contributes
        .agent_adapters
        .iter()
        .map(|adapter| adapter.id.clone())
        .collect()
}

/// The lock entry an install of `id` displaces. Only the built-in replaces a first-party entry the catalogue does not
/// vouch for; any other entry under the id is already installed.
fn displaced(lockfile: &Lockfile, id: &str, builtin: bool) -> Result<Option<LockEntry>, Error> {
    match lockfile.get(id) {
        None => Ok(None),
        Some(entry) if builtin && catalogue::is_unvouched_first_party(entry) => {
            Ok(Some(entry.clone()))
        }
        Some(_) => Err(Error::AlreadyInstalled(id.to_owned())),
    }
}

/// The manifest of an installed package, read from its unpacked folder. The folder is hashed first and must match the
/// lockfile, so a file changed after the install is never trusted, and nothing it names runs.
pub(crate) fn installed_manifest(layout: &Layout, entry: &LockEntry) -> Result<Manifest, Error> {
    let expected = entry.files_sha256.as_deref().unwrap_or(&entry.sha256);
    let files = installed_files(&entry.id, &layout.resolve(&entry.path), expected)?;
    let bytes = files
        .get(manifest::MANIFEST_FILE)
        .ok_or_else(|| Error::Tampered(entry.id.clone()))?;
    Ok(manifest::parse(bytes)?)
}

/// Reads an unpacked folder and checks its canonical hash. Any difference, or a file the folder rules refuse, is
/// tampering; a missing folder stays an I/O error.
fn installed_files(id: &str, dir: &Path, expected: &str) -> Result<Files, Error> {
    let files = match source::read_dir(dir) {
        Ok(files) => files,
        Err(error @ Error::Io { .. }) => return Err(error),
        Err(_) => return Err(Error::Tampered(id.to_owned())),
    };
    if !source::canonical_sha256(&files).eq_ignore_ascii_case(expected) {
        return Err(Error::Tampered(id.to_owned()));
    }
    Ok(files)
}

/// After unpacking, the folder must hash to exactly what was reviewed.
fn check_unpacked(id: &str, dir: &Path, expected: &str) -> Result<(), Error> {
    installed_files(id, dir, expected).map(|_| ())
}

/// Records in the install log that the installer ran before verify failed, so its CLI may still be on the host.
fn note_remaining_cli(log: &Path, command: &[String]) {
    if let Ok(mut file) = fs::OpenOptions::new().append(true).create(true).open(log) {
        let _ = writeln!(
            file,
            "verify failed after the installer ran; the CLI it installed may remain. It ran: {command:?}"
        );
    }
}

static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn describe(exit: Exit) -> String {
    match (exit.timed_out, exit.code) {
        (true, _) => "timed out and was stopped".to_owned(),
        (false, Some(code)) => format!("exited with {code}"),
        (false, None) => "ended without an exit code".to_owned(),
    }
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}
