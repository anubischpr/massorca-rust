//! The agents API agreed with the agents session: the agent kinds of installed packages and of the catalogue. It reads
//! the lockfile without a running service, so `core-agents` can call it directly.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use massorca_ext_package::manifest;
use massorca_profile::Profile;
use serde::Serialize;

use crate::catalogue;
use crate::error::{Error, HookError, codes};
use crate::host::{CommandRunner, Exit, ProgramResolver};
use crate::item::ItemState;
use crate::layout::Layout;
use crate::lockfile::Lockfile;
use crate::service::{Extensions, installed_manifest};
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

/// One agent kind of the built-in catalogue or of an installed package, with its package's state on this profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageAgentKind {
    pub kind_id: String,
    pub package_id: String,
    pub version: String,
    pub state: ItemState,
    /// The agent declaration as text, or why it cannot be used. The message names the package and never a path.
    pub decl: Result<String, String>,
}

/// Every agent kind of the built-in catalogue and of installed packages. A built-in package with an installed copy is
/// listed as that copy, at its catalogue position; third-party packages follow in lockfile order.
pub fn package_agent_kinds(profile: &Profile) -> Result<Vec<PackageAgentKind>, Error> {
    let installed = installed_agent_kinds(profile)?;
    let catalogue = catalogue_agent_kinds();
    let mut out = Vec::new();
    let mut built_in_packages: BTreeSet<&str> = BTreeSet::new();
    for kind in &catalogue {
        if !built_in_packages.insert(kind.package_id.as_str()) {
            continue;
        }
        let copies: Vec<&InstalledAgentKind> = installed
            .iter()
            .filter(|copy| copy.package_id == kind.package_id)
            .collect();
        if copies.is_empty() {
            out.extend(
                catalogue
                    .iter()
                    .filter(|built_in| built_in.package_id == kind.package_id)
                    .map(available),
            );
        } else {
            out.extend(copies.into_iter().map(installed_kind));
        }
    }
    out.extend(
        installed
            .iter()
            .filter(|copy| !built_in_packages.contains(copy.package_id.as_str()))
            .map(installed_kind),
    );
    Ok(out)
}

fn available(kind: &CatalogueAgentKind) -> PackageAgentKind {
    PackageAgentKind {
        kind_id: kind.kind_id.clone(),
        package_id: kind.package_id.clone(),
        version: kind.package_version.clone(),
        state: ItemState::Available,
        decl: std::str::from_utf8(&kind.declaration)
            .map(str::to_owned)
            .map_err(|_| format!("{}: its declaration is not text", kind.package_id)),
    }
}

fn installed_kind(kind: &InstalledAgentKind) -> PackageAgentKind {
    PackageAgentKind {
        kind_id: kind.kind_id.clone(),
        package_id: kind.package_id.clone(),
        version: kind.package_version.clone(),
        state: if kind.enabled {
            ItemState::Enabled
        } else {
            ItemState::Disabled
        },
        decl: installed_declaration(kind),
    }
}

/// The declaration of an installed kind, read from its verified folder, or why its package cannot be used.
fn installed_declaration(kind: &InstalledAgentKind) -> Result<String, String> {
    if let Some(error) = &kind.error {
        return Err(problem(&kind.package_id, error));
    }
    let unreadable = || format!("{}: its declaration cannot be read", kind.package_id);
    let path = kind.path.as_deref().ok_or_else(unreadable)?;
    fs::read_to_string(path).map_err(|_| unreadable())
}

/// Why an installed package's files cannot be used, in words that name no path.
fn problem(package_id: &str, error: &HookError) -> String {
    let cause = match error.code.as_str() {
        codes::TAMPERED => "its installed files differ from what was reviewed",
        codes::IO => "its installed files cannot be read",
        _ => "its installed manifest is invalid",
    };
    format!("{package_id}: {cause} ({})", error.code)
}

/// Adopts the built-in agent packages a profile already used, once per profile (decision 0063). The adoption needs no
/// program lookup and runs no command, so its service gets a host that does neither.
pub fn adopt_builtin_agents(
    profile: &Profile,
    used_kinds: &BTreeSet<String>,
) -> Result<Vec<String>, Error> {
    Extensions::new(profile, Arc::new(NoHost), Arc::new(NoHost)).adopt_builtin_agents(used_kinds)
}

/// The host of an adoption that needs none: no program resolves and no command runs.
struct NoHost;

impl ProgramResolver for NoHost {
    fn resolve(&self, _binary: &str) -> Option<PathBuf> {
        None
    }
}

impl CommandRunner for NoHost {
    fn run(&self, _argv: &[String], _log: &Path, _timeout: Duration) -> io::Result<Exit> {
        Err(io::Error::other("the built-in adoption runs no command"))
    }
}
