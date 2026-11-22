//! The agents API agreed with the agents session: the agent kinds of installed packages and of the catalogue. It reads
//! the lockfile without a running service, so `core-agents` can call it directly.

use std::path::PathBuf;

use massorca_ext_package::manifest;
use massorca_profile::Profile;
use serde::Serialize;

use crate::catalogue;
use crate::error::Error;
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
    /// The absolute path of `agents/<kind_id>.json` inside the unpacked package.
    pub path: PathBuf,
    pub enabled: bool,
    /// Whether this package's install ran the vendor installer for the kind's CLI.
    pub cli_installed_by_package: bool,
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

/// Every installed package's agent kinds, enabled and disabled, in lockfile order. An empty profile has none.
pub fn installed_agent_kinds(profile: &Profile) -> Result<Vec<InstalledAgentKind>, Error> {
    let layout = Layout::new(profile);
    let lockfile = Lockfile::read(&layout.lock_path())?;
    let mut out = Vec::new();
    for entry in &lockfile.packages {
        let manifest = installed_manifest(&layout, entry)?;
        let root = layout.resolve(&entry.path);
        for adapter in &manifest.contributes.agent_adapters {
            out.push(InstalledAgentKind {
                package_id: entry.id.clone(),
                package_version: entry.version.clone(),
                kind_id: adapter.id.clone(),
                path: adapter
                    .kind
                    .split('/')
                    .fold(root.clone(), |path, segment| path.join(segment)),
                enabled: entry.enabled,
                cli_installed_by_package: entry
                    .cli
                    .iter()
                    .any(|cli| cli.agent == adapter.id && cli.installed_by_package),
            });
        }
    }
    Ok(out)
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
