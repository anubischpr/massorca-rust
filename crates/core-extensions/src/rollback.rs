//! Rolls an installed package back to the version its last update replaced (`extensions.rollback`). The kept files come
//! back and the current ones are kept in their place, so a second rollback goes forward again. No vendor command runs.

use crate::error::Error;
use crate::lockfile::{KeptVersion, LockEntry};
use crate::review;
use crate::service::{Extensions, agent_kind_ids, installed_source};
use crate::source::Source;

impl Extensions {
    /// Swaps the kept version back in, under the change lock. Its review is accepted again as an update's is; a built-in
    /// needs none. Refused with `extension.no_previous_version` when no version is kept.
    pub fn rollback(&self, id: &str, consent: Option<&str>) -> Result<LockEntry, Error> {
        let _changes = self.lock_changes();
        let lockfile = self.read_lockfile()?;
        let current = lockfile
            .get(id)
            .cloned()
            .ok_or_else(|| Error::NotInstalled(id.to_owned()))?;
        self.refuse_if_busy(id)?;
        let kept = current
            .previous
            .as_deref()
            .cloned()
            .ok_or_else(|| Error::NoPreviousVersion(id.to_owned()))?;
        let target = restored(&current, &kept);
        // The kept folder must hash to what the lockfile records before its manifest is trusted.
        let manifest = self.installed_manifest(&target)?;
        if manifest.id != id {
            return Err(Error::Tampered(id.to_owned()));
        }
        let builtin = matches!(installed_source(&target), Source::Catalogue(_));
        let mut others = lockfile.clone();
        others.packages.retain(|entry| entry.id != id);
        self.check_kind_conflicts(&others, &manifest, builtin)?;
        let kinds = agent_kind_ids(&manifest);
        self.check_installing_kinds(id, &kinds)?;
        // Dropping an agent kind asks the remove guards, as an update does.
        if self
            .recorded_kinds(&current)
            .iter()
            .any(|kind| !kinds.contains(kind))
        {
            self.run_guards(id)?;
        }

        let accepted = if builtin {
            kept.consent.clone()
        } else {
            let plan = review::plan(&manifest, &*self.resolver, self.os);
            let expected = review::fingerprint(&manifest, &kept.sha256, &plan, self.os);
            match consent {
                None => return Err(Error::ConsentRequired),
                Some(consent) if consent != expected => return Err(Error::ConsentMismatch),
                Some(_) => expected,
            }
        };
        let fresh = LockEntry {
            consent: accepted,
            previous: Some(Box::new(KeptVersion::of(&current))),
            ..target
        };
        let mut lockfile = lockfile;
        if let Some(slot) = lockfile.get_mut(id) {
            *slot = fresh.clone();
        }
        lockfile.write(&self.layout.lock_path())?;
        Ok(fresh)
    }
}

/// The entry `current` becomes when `kept` is swapped back in: the files of `kept`, and everything else `current` keeps.
fn restored(current: &LockEntry, kept: &KeptVersion) -> LockEntry {
    LockEntry {
        version: kept.version.clone(),
        source: kept.source.clone(),
        sha256: kept.sha256.clone(),
        files_sha256: kept.files_sha256.clone(),
        path: kept.path.clone(),
        consent: kept.consent.clone(),
        agent_kinds: kept.agent_kinds.clone(),
        previous: None,
        ..current.clone()
    }
}
