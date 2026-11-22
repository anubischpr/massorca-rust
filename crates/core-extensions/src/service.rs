//! The extension service of one profile: review, install, enable, disable and remove. Every change is serialised by
//! one lock, and `lock.json` is committed last, so a failed change leaves nothing registered.

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use massorca_ext_package::manifest::agent::CliOs;
use massorca_ext_package::manifest::{self, Manifest};
use massorca_profile::Profile;

use crate::error::Error;
use crate::hooks::Hooks;
use crate::host::{CommandRunner, Exit, INSTALLER_TIMEOUT, ProgramResolver, current_os};
use crate::layout::Layout;
use crate::lockfile::{InstallerRun, LockCli, LockEntry, Lockfile};
use crate::review::{self, CliPlan, PlanState, Review};
use crate::source::{self, Files, Source};

/// The extension service of one profile.
pub struct Extensions {
    pub(crate) layout: Layout,
    pub(crate) resolver: Arc<dyn ProgramResolver>,
    pub(crate) runner: Arc<dyn CommandRunner>,
    pub(crate) os: Option<CliOs>,
    pub(crate) hooks: Arc<Hooks>,
    changes: Mutex<()>,
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
    pub fn install(
        &self,
        source: &Source,
        consent: Option<&str>,
        run_installer: bool,
        sha256: Option<&str>,
    ) -> Result<LockEntry, Error> {
        let _changes = self.lock_changes();
        if matches!(source, Source::File(_)) && sha256.is_none() {
            return Err(Error::HashRequired);
        }
        let package = source::load(source, sha256)?;
        let manifest = &package.manifest;
        let mut lockfile = self.read_lockfile()?;
        if lockfile.get(&manifest.id).is_some() {
            return Err(Error::AlreadyInstalled(manifest.id.clone()));
        }
        self.check_kind_conflicts(&lockfile, manifest)?;

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

        let version = manifest.version.to_string();
        let rel = Layout::package_rel_path(&manifest.id, &version, &package.sha256);
        let dir = self.layout.resolve(&rel);
        let files_sha256 = source::canonical_sha256(&package.files);
        self.unpack(&manifest.id, &package.files, &dir)?;
        let entry = check_unpacked(&manifest.id, &dir, &files_sha256)
            .and_then(|()| self.run_installers(manifest, &plan))
            .map(|cli| LockEntry {
                id: manifest.id.clone(),
                version,
                source: package.source.clone(),
                sha256: package.sha256.clone(),
                files_sha256: Some(files_sha256),
                path: rel,
                enabled: true,
                consent: expected,
                installed_at_ms: now_ms(),
                cli,
                agent_kinds: manifest
                    .contributes
                    .agent_adapters
                    .iter()
                    .map(|adapter| adapter.id.clone())
                    .collect(),
                extra: Default::default(),
            })
            .and_then(|entry| {
                lockfile.packages.push(entry.clone());
                lockfile.write(&self.layout.lock_path())?;
                Ok(entry)
            });
        if entry.is_err() {
            self.remove_unpacked(&manifest.id, &dir);
        }
        entry
    }

    /// The manifest of an installed package, read from its unpacked folder.
    pub(crate) fn installed_manifest(&self, entry: &LockEntry) -> Result<Manifest, Error> {
        installed_manifest(&self.layout, entry)
    }

    fn check_kind_conflicts(&self, lockfile: &Lockfile, manifest: &Manifest) -> Result<(), Error> {
        for entry in &lockfile.packages {
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
                    let verified = self
                        .runner
                        .run(&cli.verify, &log, INSTALLER_TIMEOUT)
                        .map_err(|error| unverified(error.to_string()))?;
                    if !verified.success() {
                        return Err(unverified(describe(verified)));
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
