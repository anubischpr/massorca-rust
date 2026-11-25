//! The extension service of one profile: review, install, enable, disable and remove. Every change is serialised by
//! one lock, and `lock.json` is committed last, so a failed change leaves nothing registered.

use std::collections::{BTreeMap, BTreeSet};
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

use crate::agents::catalogue_agent_kinds;
use crate::catalogue;
use crate::error::{Diagnosis, Error};
use crate::hooks::Hooks;
use crate::host::{CommandRunner, Exit, INSTALLER_TIMEOUT, ProgramResolver, current_os};
use crate::install_log_tail::last_log_lines;
use crate::layout::Layout;
use crate::lockfile::{
    InstallerRun, LockCli, LockEntry, LockSource, Lockfile, SourceKind, sync_dir,
};
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
            let entry = staged.entry(cli, false);
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

    /// Updates an installed package to what its source holds now, in place (decision 0062, Updates). Its enabled state,
    /// CLI registrations, install time, adoption mark and unknown fields are kept, and no vendor command runs. `consent`
    /// is the fingerprint of the new review, as for an install; a built-in needs none. `sha256` pins an archive.
    /// Returns `extension.up_to_date` when the source holds the installed package already.
    pub fn update(
        &self,
        id: &str,
        consent: Option<&str>,
        sha256: Option<&str>,
    ) -> Result<LockEntry, Error> {
        let (old, staged) = self.stage_update(id, consent, sha256)?;
        self.commit_update(&old, staged)
    }

    /// Checks the new package and unpacks it, under the change lock. The returned mark keeps the package busy until the
    /// update is committed or abandoned.
    fn stage_update(
        &self,
        id: &str,
        consent: Option<&str>,
        sha256: Option<&str>,
    ) -> Result<(LockEntry, Staged), Error> {
        let _changes = self.lock_changes();
        let lockfile = self.read_lockfile()?;
        let old = lockfile
            .get(id)
            .cloned()
            .ok_or_else(|| Error::NotInstalled(id.to_owned()))?;
        self.refuse_if_busy(id)?;
        let source = installed_source(&old);
        if matches!(source, Source::File(_)) && sha256.is_none() {
            return Err(Error::HashRequired);
        }
        let package = source::load(&source, sha256)?;
        if package.manifest.id != id {
            // The new entry takes its id from the manifest, so another id would replace or orphan the wrong entry.
            return Err(Error::SourceIdChanged {
                id: id.to_owned(),
                found: package.manifest.id.clone(),
            });
        }
        if package.sha256.eq_ignore_ascii_case(&old.sha256) {
            return Err(Error::UpToDate(id.to_owned()));
        }
        let builtin = matches!(source, Source::Catalogue(_));
        let mut others = lockfile.clone();
        others.packages.retain(|entry| entry.id != id);
        self.check_kind_conflicts(&others, &package.manifest, builtin)?;
        let kinds = agent_kind_ids(&package.manifest);
        self.check_installing_kinds(id, &kinds)?;

        // The built-in is vouched for by the build, as in refresh and adoption; any other package needs the new review.
        let accepted = if builtin {
            old.consent.clone()
        } else {
            let plan = review::plan(&package.manifest, &*self.resolver, self.os);
            let expected = review::fingerprint(&package.manifest, &package.sha256, &plan, self.os);
            match consent {
                None => return Err(Error::ConsentRequired),
                Some(consent) if consent != expected => return Err(Error::ConsentMismatch),
                Some(_) => expected,
            }
        };
        // Dropping an agent kind asks the remove guards, as removing its package would.
        if self
            .recorded_kinds(&old)
            .iter()
            .any(|kind| !kinds.contains(kind))
        {
            self.run_guards(id)?;
        }

        let busy = self.mark_busy(id, kinds)?;
        let staged = self.unpack_staged(package, busy, Vec::new(), accepted, builtin)?;
        Ok((old, staged))
    }

    /// Records the update under the change lock, after the new folder is in place. Whatever fails here removes that
    /// folder, so the installed package stays as it was.
    fn commit_update(&self, old: &LockEntry, staged: Staged) -> Result<LockEntry, Error> {
        let _changes = self.lock_changes();
        match self.write_update(old, &staged) {
            Ok(entry) => {
                if old.path != entry.path {
                    // The replaced entry left the lockfile with this update, so its folder is only litter now.
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

    /// Replaces the entry of `old` with the staged one, keeping what an update keeps, and writes the lockfile last.
    fn write_update(&self, old: &LockEntry, staged: &Staged) -> Result<LockEntry, Error> {
        let mut lockfile = self.read_lockfile()?;
        let current = lockfile
            .get(&old.id)
            .ok_or_else(|| Error::NotInstalled(old.id.clone()))?;
        if current.path != old.path {
            // A built-in refresh moved the entry while this update was staged; nothing is overwritten blindly.
            return Err(Error::Busy(old.id.clone()));
        }
        let fresh = LockEntry {
            enabled: current.enabled,
            source: current.source.clone(),
            installed_at_ms: current.installed_at_ms,
            extra: current.extra.clone(),
            ..staged.entry(current.cli.clone(), current.adopted)
        };
        if let Some(slot) = lockfile
            .packages
            .iter_mut()
            .find(|entry| entry.id == fresh.id)
        {
            *slot = fresh.clone();
        }
        lockfile.write(&self.layout.lock_path())?;
        Ok(fresh)
    }

    /// Adopts the built-in agent packages that declare one of `used_kinds`, once per profile (decision 0063). Each is
    /// registered enabled, with no vendor command, no program lookup and no consent: the build vouches for it. One
    /// lockfile write records them and the run, so a later call changes nothing. Returns the adopted package ids in
    /// catalogue order.
    pub fn adopt_builtin_agents(
        &self,
        used_kinds: &BTreeSet<String>,
    ) -> Result<Vec<String>, Error> {
        let _changes = self.lock_changes();
        let mut lockfile = self.read_lockfile()?;
        if lockfile.has_migration(BUILTIN_AGENTS_MIGRATION) {
            return Ok(Vec::new());
        }
        let mut staged: Vec<Staged> = Vec::new();
        for id in adoption_ids(used_kinds, &lockfile) {
            match self.stage_built_in(&lockfile, &id) {
                Ok(next) => staged.push(next),
                Err(error) => {
                    self.remove_staged(&staged);
                    return Err(error);
                }
            }
        }
        let entries: Vec<LockEntry> = staged
            .iter()
            .map(|next| next.entry(Vec::new(), true))
            .collect();
        let replaced: Vec<String> = lockfile
            .packages
            .iter()
            .filter(|entry| entries.iter().any(|new| new.id == entry.id))
            .map(|entry| entry.path.clone())
            .collect();
        lockfile
            .packages
            .retain(|entry| !entries.iter().any(|new| new.id == entry.id));
        lockfile.packages.extend(entries.iter().cloned());
        lockfile.record_migration(BUILTIN_AGENTS_MIGRATION);
        if let Err(error) = lockfile.write(&self.layout.lock_path()) {
            self.remove_staged(&staged);
            return Err(error);
        }
        for path in replaced {
            if !entries.iter().any(|entry| entry.path == path) {
                // The unvouched entry left the lockfile with this write, so its folder is only litter now.
                let _ = fs::remove_dir_all(self.layout.resolve(&path));
            }
        }
        Ok(entries.into_iter().map(|entry| entry.id).collect())
    }

    /// Brings every built-in install whose files predate this build's embedded package up to date, in place
    /// (decision 0063). Its enabled state, consent, CLI registrations, install time and adoption mark are kept, and no
    /// vendor command or program lookup runs. One lockfile write records every package that staged; a package that
    /// fails to stage stays as it was, and its first error is returned after that write. Returns the refreshed
    /// package ids in lockfile order.
    pub fn refresh_builtins(&self) -> Result<Vec<String>, Error> {
        let _changes = self.lock_changes();
        let mut lockfile = self.read_lockfile()?;
        let mut outdated: Vec<LockEntry> = Vec::new();
        for entry in &lockfile.packages {
            // Only the first entry of an id is ever read, so only it is refreshed.
            if catalogue::is_outdated_builtin_install(entry)
                && !outdated.iter().any(|old| old.id == entry.id)
            {
                outdated.push(entry.clone());
            }
        }
        let mut staged: Vec<(LockEntry, Staged)> = Vec::new();
        let mut failed: Option<Error> = None;
        for old in outdated {
            match self.stage_refresh(&lockfile, &old) {
                Ok(Some(next)) => staged.push((old, next)),
                Ok(None) => {}
                Err(error) => {
                    failed.get_or_insert(error);
                }
            }
        }
        if staged.is_empty() {
            return failed.map_or(Ok(Vec::new()), Err);
        }
        for (old, next) in &staged {
            let fresh = LockEntry {
                enabled: old.enabled,
                consent: old.consent.clone(),
                installed_at_ms: old.installed_at_ms,
                extra: old.extra.clone(),
                ..next.entry(old.cli.clone(), old.adopted)
            };
            if let Some(slot) = lockfile
                .packages
                .iter_mut()
                .find(|entry| entry.id == old.id)
            {
                *slot = fresh;
            }
        }
        if let Err(error) = lockfile.write(&self.layout.lock_path()) {
            for (_, next) in &staged {
                self.remove_unpacked(&next.manifest.id, &next.dir);
            }
            return Err(error);
        }
        for (old, _) in &staged {
            // The outdated files left the lockfile with this write, so their folder is only litter now.
            let _ = fs::remove_dir_all(self.layout.resolve(&old.path));
        }
        match failed {
            Some(error) => Err(error),
            None => Ok(staged.into_iter().map(|(old, _)| old.id).collect()),
        }
    }

    /// Unpacks the embedded package that replaces an outdated built-in install. `None` when the new folder would be the
    /// old one: unpacking replaces the target folder, which here is the install's only copy.
    fn stage_refresh(&self, lockfile: &Lockfile, old: &LockEntry) -> Result<Option<Staged>, Error> {
        let package = source::load(&Source::Catalogue(old.id.clone()), None)?;
        let rel = Layout::package_rel_path(
            &package.manifest.id,
            &package.manifest.version.to_string(),
            &package.sha256,
        );
        if rel == old.path {
            return Ok(None);
        }
        self.stage_loaded_built_in(lockfile, package).map(Some)
    }

    /// Checks and unpacks one built-in package for adoption. Callers hold the change lock.
    fn stage_built_in(&self, lockfile: &Lockfile, id: &str) -> Result<Staged, Error> {
        let package = source::load(&Source::Catalogue(id.to_owned()), None)?;
        self.stage_loaded_built_in(lockfile, package)
    }

    /// Checks and unpacks a loaded built-in package. Callers hold the change lock.
    fn stage_loaded_built_in(
        &self,
        lockfile: &Lockfile,
        package: LoadedPackage,
    ) -> Result<Staged, Error> {
        self.check_kind_conflicts(lockfile, &package.manifest, true)?;
        let kinds = agent_kind_ids(&package.manifest);
        let id = package.manifest.id.clone();
        self.check_installing_kinds(&id, &kinds)?;
        let busy = self.mark_busy(&id, kinds)?;
        self.unpack_staged(package, busy, Vec::new(), String::new(), true)
    }

    /// Deletes the folders of packages staged for an adoption that did not commit.
    fn remove_staged(&self, staged: &[Staged]) {
        for next in staged {
            self.remove_unpacked(&next.manifest.id, &next.dir);
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

    /// The agent kinds an installed package records. An entry from before kinds were recorded is read from its folder,
    /// best-effort: an update replaces a tampered folder anyway.
    fn recorded_kinds(&self, entry: &LockEntry) -> Vec<String> {
        if !entry.agent_kinds.is_empty() {
            return entry.agent_kinds.clone();
        }
        self.installed_manifest(entry)
            .map_or_else(|_| Vec::new(), |manifest| agent_kind_ids(&manifest))
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
            let mut folders = BTreeSet::from([staging.clone()]);
            for (path, bytes) in files {
                let target = path
                    .split('/')
                    .fold(staging.clone(), |target, segment| target.join(segment));
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(Error::io("cannot create", parent))?;
                    folders.extend(
                        parent
                            .ancestors()
                            .take_while(|folder| folder.starts_with(&staging))
                            .map(Path::to_path_buf),
                    );
                }
                // A name that collides with one already written (an alias) fails the install.
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&target)
                    .and_then(|mut file| file.write_all(bytes).and_then(|()| file.sync_all()))
                    .map_err(Error::io("cannot write", &target))?;
            }
            // The lockfile is committed after this, so the files it names must be on disk before it, not after a crash.
            for folder in &folders {
                sync_dir(folder).map_err(Error::io("cannot sync", folder))?;
            }
            if dir.exists() {
                // A leftover from an install that never reached the lockfile.
                fs::remove_dir_all(dir).map_err(Error::io("cannot replace", dir))?;
            }
            fs::rename(&staging, dir).map_err(Error::io("cannot move into place", dir))?;
            let root = self.layout.package_root(id);
            // `pkgs/<id>` may be new, so its own entry in `pkgs/` is synced too.
            let synced = [root.as_path(), root.parent().unwrap_or(&root)]
                .into_iter()
                .try_for_each(|folder| sync_dir(folder).map_err(Error::io("cannot sync", folder)));
            if synced.is_err() {
                let _ = fs::remove_dir_all(dir);
            }
            synced
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

    /// What a failed command was looked for and where, with the last lines of its log (decision 0092).
    fn diagnosis(&self, programs: Vec<String>, log: Option<&Path>) -> Box<Diagnosis> {
        Box::new(Diagnosis {
            programs,
            search: self.resolver.search(),
            output_tail: log.map(last_log_lines).unwrap_or_default(),
        })
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
                PlanState::WillRun { command, .. } => {
                    if !is_vendor_program(command) {
                        return Err(Error::InstallerFailed {
                            agent: adapter.id.clone(),
                            message: "its program is not on the vendor allowlist".to_owned(),
                            diagnosis: self.diagnosis(program_of(command), None),
                        });
                    }
                    let data = self.layout.data_dir(&manifest.id);
                    fs::create_dir_all(&data).map_err(Error::io("cannot create", &data))?;
                    let log = self.layout.install_log(&manifest.id);
                    let ran_at_ms = now_ms();
                    let failed = |message: String| Error::InstallerFailed {
                        agent: adapter.id.clone(),
                        message,
                        diagnosis: self.diagnosis(program_of(command), Some(&log)),
                    };
                    let exit = self
                        .runner
                        .run(command, &log, INSTALLER_TIMEOUT)
                        .map_err(|error| failed(error.to_string()))?;
                    if !exit.success() {
                        return Err(failed(describe(exit)));
                    }
                    let unverified = |message: String, programs: Vec<String>| {
                        note_remaining_cli(&log, command);
                        Error::VerifyFailed {
                            agent: adapter.id.clone(),
                            message,
                            installer: Some(command.clone()),
                            diagnosis: self.diagnosis(programs, Some(&log)),
                        }
                    };
                    // Without a verify command the install succeeds when the binary resolves.
                    if let Some(verify) = &cli.verify {
                        let verified = self
                            .runner
                            .run(verify, &log, INSTALLER_TIMEOUT)
                            .map_err(|error| unverified(error.to_string(), program_of(verify)))?;
                        if !verified.success() {
                            return Err(unverified(describe(verified), program_of(verify)));
                        }
                    }
                    if self.resolver.resolve(&cli.binary).is_none() {
                        return Err(unverified(
                            format!("`{}` does not resolve", cli.binary),
                            vec![cli.binary.clone()],
                        ));
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

/// The program a command names: its first word.
fn program_of(argv: &[String]) -> Vec<String> {
    argv.first().cloned().into_iter().collect()
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
    fn entry(&self, cli: Vec<LockCli>, adopted: bool) -> LockEntry {
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
            adopted,
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

/// The source an installed package was read from, as its lock entry names it.
fn installed_source(entry: &LockEntry) -> Source {
    let reference = entry.source.reference.clone();
    match entry.source.kind {
        SourceKind::Catalogue => Source::Catalogue(reference),
        SourceKind::Dir => Source::Dir(PathBuf::from(reference)),
        SourceKind::File => Source::File(PathBuf::from(reference)),
    }
}

/// The built-in packages an adoption registers: those declaring a used kind and not installed as built-ins, in catalogue
/// order.
fn adoption_ids(used_kinds: &BTreeSet<String>, lockfile: &Lockfile) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for kind in catalogue_agent_kinds() {
        let installed = lockfile
            .get(&kind.package_id)
            // An outdated built-in is still the person's install: refresh_builtins updates it, adoption must not reset it.
            .is_some_and(|entry| {
                catalogue::is_builtin_install(entry)
                    || catalogue::is_outdated_builtin_install(entry)
            });
        if used_kinds.contains(&kind.kind_id) && !installed && !ids.contains(&kind.package_id) {
            ids.push(kind.package_id);
        }
    }
    ids
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

/// The lockfile migration that adopts the built-in agent packages (decision 0063).
const BUILTIN_AGENTS_MIGRATION: &str = "builtin-agents";

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
