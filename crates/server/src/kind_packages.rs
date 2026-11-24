//! The agent kinds the profile's extension packages provide, with their real state (decisions 0059 and 0063).

use std::collections::BTreeSet;

use massorca_core_agents::{KindPackages, PackageKind, PackageState, used_kinds};
use massorca_core_extensions::item::ItemState;
use massorca_core_extensions::{Extensions, catalogue_agent_kinds, package_agent_kinds};
use massorca_core_store::Store;
use massorca_profile::Profile;

/// Reads the profile's lockfile on every catalog load, so an install, a disable or a removal applies to the next run.
#[derive(Debug)]
pub(crate) struct ExtensionKindPackages {
    profile: Profile,
}

impl ExtensionKindPackages {
    pub(crate) fn new(profile: Profile) -> Self {
        Self { profile }
    }
}

impl KindPackages for ExtensionKindPackages {
    fn kinds(&self) -> Vec<PackageKind> {
        match package_agent_kinds(&self.profile) {
            Ok(kinds) => kinds
                .into_iter()
                .map(|kind| PackageKind {
                    kind_id: kind.kind_id,
                    package_id: kind.package_id,
                    version: kind.version,
                    state: package_state(kind.state),
                    decl: kind.decl,
                })
                .collect(),
            // No package state can be trusted: the built-ins are listed in an unknown state and none of them runs.
            Err(error) => catalogue_agent_kinds()
                .into_iter()
                .map(|kind| PackageKind {
                    decl: Err(format!(
                        "{}: the extensions lockfile cannot be read ({})",
                        kind.package_id,
                        error.code()
                    )),
                    kind_id: kind.kind_id,
                    package_id: kind.package_id,
                    version: kind.package_version,
                    state: PackageState::Unknown,
                })
                .collect(),
        }
    }
}

fn package_state(state: ItemState) -> PackageState {
    match state {
        ItemState::Available => PackageState::Available,
        ItemState::Enabled => PackageState::Enabled,
        ItemState::Disabled => PackageState::Disabled,
    }
}

/// Puts the built-in agent packages in order before requests are served: refreshes the ones an older build installed,
/// then adopts, once per profile, the ones whose kind the profile already used. A failure is logged and retried at the
/// next start; a run of a kind that is not enabled is refused meanwhile, never started silently.
pub(crate) fn prepare_builtin_agents(extensions: &Extensions, store: &Store, profile: &Profile) {
    // Requests are not served yet, so no install is in flight whose unreferenced folder this could remove.
    match extensions.sweep_leftovers() {
        Ok(removed) if !removed.is_empty() => {
            tracing::info!(paths = ?removed, "extension install leftovers removed");
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "extension install leftovers were not swept"),
    }
    match extensions.refresh_builtins() {
        Ok(refreshed) if !refreshed.is_empty() => {
            tracing::info!(packages = ?refreshed, "built-in agent packages refreshed");
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "built-in agent packages were not refreshed"),
    }
    let used: BTreeSet<String> = match used_kinds(store, profile.dir()) {
        Ok(used) => used,
        Err(error) => {
            tracing::warn!(%error, "the used agent kinds cannot be read; adoption waits for the next start");
            return;
        }
    };
    match extensions.adopt_builtin_agents(&used) {
        Ok(adopted) if !adopted.is_empty() => {
            tracing::info!(packages = ?adopted, "built-in agent packages adopted");
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "built-in agent packages were not adopted"),
    }
}
