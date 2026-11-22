//! The agents API agreed with the agents session: the agent kinds of installed packages and of the catalogue. It reads
//! the lockfile without a running service, so `core-agents` can call it directly.

use std::path::PathBuf;

use massorca_ext_package::manifest;
use massorca_profile::Profile;
use serde::Serialize;

use crate::catalogue;
use crate::error::{Error, HookError};
use crate::layout::Layout;
use crate::lockfile::Lockfile;
use crate::service::installed_manifest;
use crate::views::CliSpec;

/// One agent kind of an installed package, enabled or not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstalledAgentKind {
    pub package_id: String,
    pub package_version: String,
    pub kind_id: String,
    /// The absolute path of `agents/<kind_id>.json` inside the unpacked package; `None` when `error` is set.
    pub path: Option<PathBuf>,
    pub enabled: bool,
    /// Whether this package's install ran the vendor installer for the kind's CLI.
    pub cli_installed_by_package: bool,
    /// Why the package's files cannot be used, such as `extension.tampered`. Its kinds are listed but not startable.
    pub error: Option<HookError>,
}

/// One agent kind of a catalogue package: its declaration bytes and its `cli` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueAgentKind {
    pub package_id: String,
    pub package_version: String,
    pub kind_id: String,
    pub declaration: Vec<u8>,
    pub cli: CliSpec,
}

/// Every installed package's agent kinds, enabled and disabled, in lockfile order. An empty profile has none. A
/// package whose files cannot be trusted never hides the others: its kinds come back with `error` set and no `path`.
/// Only an unreadable lockfile fails the whole call.
pub fn installed_agent_kinds(profile: &Profile) -> Result<Vec<InstalledAgentKind>, Error> {
    let layout = Layout::new(profile);
    let lockfile = Lockfile::read(&layout.lock_path())?;
    let mut out = Vec::new();
    for entry in &lockfile.packages {
        // A first-party id the catalogue does not vouch for never contributes a kind: it could impersonate a built-in.
        if catalogue::is_unvouched_first_party(entry) {
            continue;
        }
        let manifest = match installed_manifest(&layout, entry) {
            Ok(manifest) => manifest,
            Err(error) => {
                let failure = HookError::new(error.code(), error.to_string());
                out.extend(recorded_kinds(entry).map(|kind_id| {
                    InstalledAgentKind {
                        package_id: entry.id.clone(),
                        package_version: entry.version.clone(),
                        cli_installed_by_package: entry
                            .cli
                            .iter()
                            .any(|cli| cli.agent == kind_id && cli.installed_by_package),
                        kind_id,
                        path: None,
                        enabled: entry.enabled,
                        error: Some(failure.clone()),
                    }
                }));
                continue;
            }
        };
        let root = layout.resolve(&entry.path);
        for adapter in &manifest.contributes.agent_adapters {
            out.push(InstalledAgentKind {
                package_id: entry.id.clone(),
                package_version: entry.version.clone(),
                kind_id: adapter.id.clone(),
                path: Some(
                    adapter
                        .kind
                        .split('/')
                        .fold(root.clone(), |path, segment| path.join(segment)),
                ),
                enabled: entry.enabled,
                cli_installed_by_package: entry
                    .cli
                    .iter()
                    .any(|cli| cli.agent == adapter.id && cli.installed_by_package),
                error: None,
            });
        }
    }
    Ok(out)
}

/// The kind ids the lockfile recorded for a package; an entry written before `agent_kinds` existed falls back to its
/// CLI records.
fn recorded_kinds(entry: &crate::lockfile::LockEntry) -> impl Iterator<Item = String> + '_ {
    let fallback = entry.agent_kinds.is_empty();
    entry.agent_kinds.iter().cloned().chain(
        entry
            .cli
            .iter()
            .filter(move |_| fallback)
            .map(|cli| cli.agent.clone()),
    )
}

/// Every catalogue package's agent kinds, in catalogue order. Each catalogue adapter has a `cli` block (tested).
pub fn catalogue_agent_kinds() -> Vec<CatalogueAgentKind> {
    let mut out = Vec::new();
    for entry in catalogue::entries() {
        let Some(manifest) = entry
            .file(manifest::MANIFEST_FILE)
            .and_then(|bytes| manifest::load(bytes).ok())
        else {
            continue;
        };
        for adapter in &manifest.contributes.agent_adapters {
            let (Some(cli), Some(declaration)) = (&adapter.cli, entry.file(&adapter.kind)) else {
                continue;
            };
            out.push(CatalogueAgentKind {
                package_id: manifest.id.clone(),
                package_version: manifest.version.to_string(),
                kind_id: adapter.id.clone(),
                declaration: declaration.to_vec(),
                cli: CliSpec::of(cli),
            });
        }
    }
    out
}
