//! The agents API agreed with the agents session: the agent kinds of installed packages and of the catalogue. It reads
//! the lockfile without a running service, so `core-agents` can call it directly.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use massorca_ext_package::manifest;
use massorca_profile::Profile;
use serde::Serialize;

use crate::catalogue;
use crate::error::{Error, HookError, codes};
use crate::item::ItemState;
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
///
/// A package being updated while this reads can show files that no longer match the entry read: the update writes the
/// new lockfile, then deletes the old folder. So when a package reads as unusable and the lockfile changed meanwhile,
/// the read starts again from the new lockfile; a lockfile that keeps changing reports those packages as being updated
/// (`extension.busy`) instead of tampered.
pub fn installed_agent_kinds(profile: &Profile) -> Result<Vec<InstalledAgentKind>, Error> {
    let layout = Layout::new(profile);
    settled(
        || Lockfile::read(&layout.lock_path()),
        |lockfile| kinds_of(&layout, lockfile),
    )
}

/// How many times a read that found an unusable package starts again because the lockfile changed meanwhile.
const UPDATE_RETRIES: usize = 3;

/// The kinds `kinds` reads from the lockfile `read` returns, read again while a package reads as unusable and the
/// lockfile changed meanwhile (see [`installed_agent_kinds`]).
fn settled(
    mut read: impl FnMut() -> Result<Lockfile, Error>,
    kinds: impl Fn(&Lockfile) -> Vec<InstalledAgentKind>,
) -> Result<Vec<InstalledAgentKind>, Error> {
    let mut lockfile = read()?;
    let mut retries = 0;
    loop {
        let mut out = kinds(&lockfile);
        if out.iter().all(|kind| kind.error.is_none()) {
            return Ok(out);
        }
        let now = read()?;
        if now == lockfile {
            return Ok(out);
        }
        if retries == UPDATE_RETRIES {
            for kind in &mut out {
                if let Some(error) = &mut kind.error {
                    *error = HookError::new(
                        codes::BUSY,
                        format!("package {} is being updated; try again", kind.package_id),
                    );
                }
            }
            return Ok(out);
        }
        retries += 1;
        lockfile = now;
    }
}

/// Every installed package's agent kinds as `lockfile` records them, each package's files checked against its entry.
fn kinds_of(layout: &Layout, lockfile: &Lockfile) -> Vec<InstalledAgentKind> {
    let mut out = Vec::new();
    for entry in &lockfile.packages {
        // A first-party id the catalogue does not vouch for never contributes a kind: it could impersonate a built-in.
        if catalogue::is_unvouched_first_party(entry) {
            continue;
        }
        let manifest = match installed_manifest(layout, entry) {
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
    out
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
        codes::BUSY => "it is being updated; try again",
        _ => "its installed manifest is invalid",
    };
    format!("{package_id}: {cause} ({})", error.code)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// A lockfile told apart by one migration name.
    fn lockfile(mark: &str) -> Lockfile {
        Lockfile {
            migrations: vec![mark.to_owned()],
            ..Lockfile::default()
        }
    }

    /// One kind of package `ext.fake`, unusable (tampered) while the lockfile is marked `broken`.
    fn kinds(lockfile: &Lockfile) -> Vec<InstalledAgentKind> {
        let broken = lockfile.migrations.iter().any(|mark| mark.starts_with("broken"));
        vec![InstalledAgentKind {
            package_id: "ext.fake".to_owned(),
            package_version: "1.0.0".to_owned(),
            kind_id: "fake".to_owned(),
            path: None,
            enabled: true,
            cli_installed_by_package: false,
            error: broken.then(|| HookError::new(codes::TAMPERED, "tampered")),
        }]
    }

    /// Reads that return `marks` in order, then the last one forever.
    fn reads(marks: &[&str]) -> impl FnMut() -> Result<Lockfile, Error> {
        let left = RefCell::new(marks.iter().map(|mark| lockfile(mark)).collect::<Vec<_>>());
        move || {
            let mut left = left.borrow_mut();
            Ok(if left.len() > 1 {
                left.remove(0)
            } else {
                left[0].clone()
            })
        }
    }

    fn code(kinds: &[InstalledAgentKind]) -> Option<String> {
        kinds[0].error.as_ref().map(|error| error.code.clone())
    }

    #[test]
    fn a_package_updated_during_the_read_is_read_again_from_the_new_lockfile() {
        let out = settled(reads(&["broken", "updated"]), kinds).unwrap();
        assert_eq!(code(&out), None, "the new lockfile's files match");
    }

    #[test]
    fn a_package_whose_lockfile_did_not_change_stays_tampered() {
        let out = settled(reads(&["broken"]), kinds).unwrap();
        assert_eq!(code(&out).as_deref(), Some(codes::TAMPERED));
    }

    #[test]
    fn a_package_whose_lockfile_keeps_changing_is_reported_as_being_updated() {
        let marks: Vec<String> = (0..=UPDATE_RETRIES + 1).map(|n| format!("broken-{n}")).collect();
        let marks: Vec<&str> = marks.iter().map(String::as_str).collect();
        let out = settled(reads(&marks), kinds).unwrap();
        assert_eq!(code(&out).as_deref(), Some(codes::BUSY));
        let problem = installed_declaration(&out[0]).unwrap_err();
        assert!(problem.contains("being updated"), "{problem}");
        assert!(!problem.contains("differ"), "{problem}");
    }

    #[test]
    fn a_usable_package_is_read_once() {
        let mut count = 0;
        let out = settled(
            || {
                count += 1;
                Ok(lockfile("fine"))
            },
            kinds,
        )
        .unwrap();
        assert_eq!(code(&out), None);
        assert_eq!(count, 1);
    }
}
