//! The data of the nine-part Marketplace item page, for an installed package or a catalogue entry, and the listed
//! images it shows. There is no item-specific code: everything comes from the manifest, the listing and the state.

use std::fs;
use std::path::PathBuf;

use massorca_ext_package::manifest::agent::AgentCli;
use massorca_ext_package::manifest::settings::{SettingSection, SettingType};
use massorca_ext_package::manifest::{self, Manifest};
use massorca_ext_package::risk;
use massorca_ext_package::uninstall::{Offer, uninstall_effects};
use serde::Serialize;

use crate::catalogue;
use crate::error::Error;
use crate::lockfile::{LockCli, LockEntry, LockSource};
use crate::service::{Extensions, installed_manifest};
use crate::views::{CliSpec, ListingView, PermissionView, RiskView};

/// The largest image `asset` returns.
pub const MAX_ASSET_BYTES: u64 = 1024 * 1024;

/// One Marketplace item.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub publisher: String,
    pub state: ItemState,
    pub installed: Option<InstalledInfo>,
    /// Logo, category, developer, mention, banner and connections.
    pub listing: ListingView,
    pub permissions: Vec<PermissionView>,
    pub risk: RiskView,
    pub readme: Option<String>,
    pub agents: Vec<AgentItem>,
    /// For a tool package: each host program, with where it resolves on this host (decision 0151).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub programs: Vec<AgentItem>,
    /// For an agent or tool package: whether every agent's CLI and every host program resolves on this host.
    pub detected: Option<bool>,
    /// Settings with their section (`basic` or `advanced`).
    pub settings: Vec<SettingItem>,
    /// The Danger Zone.
    pub uninstall: UninstallItem,
    /// Set when the installed files cannot be trusted: the item then carries only what the lockfile records.
    pub problem: Option<ItemProblem>,
}

/// Why an installed package's files cannot be trusted, with the protocol code and the message to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemProblem {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemState {
    Available,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InstalledInfo {
    pub version: String,
    pub source: LockSource,
    pub sha256: String,
    pub installed_at_ms: u64,
    /// What the install did about each agent CLI, including the vendor command that ran.
    pub cli: Vec<LockCli>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentItem {
    pub id: String,
    pub detected: bool,
    pub program_path: Option<PathBuf>,
    /// The search path the CLI was looked for on: `process` or `registry` (decision 0092). Absent without a CLI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_source: Option<String>,
    pub cli_installed_by_package: bool,
    pub cli: Option<CliSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SettingItem {
    pub key: String,
    pub title: String,
    #[serde(rename = "type")]
    pub value_type: &'static str,
    pub section: &'static str,
    pub default: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UninstallItem {
    pub package_files: bool,
    pub storage: bool,
    pub settings: Vec<String>,
    pub secret_names: Vec<String>,
    pub secrets_unscoped: bool,
    pub connections: Vec<String>,
    pub cli_binaries: Vec<String>,
    /// Agent credential homes, deleted only when the person asks.
    pub offered_agent_homes: Vec<String>,
}

impl Extensions {
    /// Every catalogue entry, with its state on this profile.
    pub fn catalogue(&self) -> Result<Vec<Item>, Error> {
        let lockfile = self.read_lockfile()?;
        catalogue::entries()
            .iter()
            .map(|entry| {
                let bytes = entry
                    .file(manifest::MANIFEST_FILE)
                    .ok_or_else(|| Error::NotFound(entry.id.to_owned()))?;
                // Only the built-in's own install counts as installed; a lock entry under the same id does not.
                let installed = lockfile
                    .get(entry.id)
                    .filter(|lock| catalogue::is_builtin_install(lock));
                Ok(self.item(&manifest::load(bytes)?, installed))
            })
            .collect()
    }

    /// Every installed package, in lockfile order. A package whose files cannot be trusted never fails the others.
    pub fn list(&self) -> Result<Vec<Item>, Error> {
        let lockfile = self.read_lockfile()?;
        Ok(lockfile
            .packages
            .iter()
            .map(|entry| self.installed_item_or_problem(entry))
            .collect())
    }

    /// One item, installed or from the catalogue. An installed package whose files cannot be trusted comes back with
    /// its problem instead of an error.
    pub fn get(&self, id: &str) -> Result<Item, Error> {
        let lockfile = self.read_lockfile()?;
        if let Some(entry) = lockfile.get(id) {
            return Ok(self.installed_item_or_problem(entry));
        }
        let bytes = catalogue::find(id)
            .and_then(|entry| entry.file(manifest::MANIFEST_FILE))
            .ok_or_else(|| Error::NotFound(id.to_owned()))?;
        Ok(self.item(&manifest::load(bytes)?, None))
    }

    /// A listed image (logo or banner) of an item, at most [`MAX_ASSET_BYTES`].
    pub fn asset(&self, id: &str, path: &str) -> Result<Vec<u8>, Error> {
        let lockfile = self.read_lockfile()?;
        if let Some(entry) = lockfile
            .get(id)
            .filter(|entry| catalogue::is_unvouched_first_party(entry))
        {
            return Err(Error::ReservedIdentity {
                id: entry.id.clone(),
                reference: entry.source.reference.clone(),
            });
        }
        // An installed package's files must be trusted before any of them is served.
        let item = match lockfile.get(id) {
            Some(entry) => self.installed_item(entry)?,
            None => self.get(id)?,
        };
        if !item.listing.images().any(|image| image == path) {
            return Err(Error::AssetNotListed(path.to_owned()));
        }
        let too_large = |size: u64| Error::AssetTooLarge {
            path: path.to_owned(),
            size,
            limit: MAX_ASSET_BYTES,
        };
        if let Some(entry) = lockfile.get(id) {
            let file = self.layout.resolve(&entry.path).join(path);
            let size = fs::metadata(&file)
                .map_err(Error::io("cannot read", &file))?
                .len();
            if size > MAX_ASSET_BYTES {
                return Err(too_large(size));
            }
            return fs::read(&file).map_err(Error::io("cannot read", &file));
        }
        let bytes = catalogue::find(id)
            .and_then(|entry| entry.file(path))
            .ok_or_else(|| Error::AssetNotListed(path.to_owned()))?;
        if bytes.len() as u64 > MAX_ASSET_BYTES {
            return Err(too_large(bytes.len() as u64));
        }
        Ok(bytes.to_vec())
    }

    /// An installed package from its manifest. Fails when its files cannot be trusted.
    fn installed_item(&self, entry: &LockEntry) -> Result<Item, Error> {
        Ok(self.item(&installed_manifest(&self.layout, entry)?, Some(entry)))
    }

    /// An installed package from its manifest, or, when its files cannot be trusted, its lockfile record and problem.
    fn installed_item_or_problem(&self, entry: &LockEntry) -> Item {
        if catalogue::is_unvouched_first_party(entry) {
            let error = Error::ReservedIdentity {
                id: entry.id.clone(),
                reference: entry.source.reference.clone(),
            };
            return lockfile_item(entry, &error);
        }
        self.installed_item(entry)
            .unwrap_or_else(|error| lockfile_item(entry, &error))
    }

    fn item(&self, manifest: &Manifest, entry: Option<&LockEntry>) -> Item {
        let installed_by_package = |agent: &str| {
            entry.is_some_and(|entry| {
                entry
                    .cli
                    .iter()
                    .any(|cli| cli.agent == agent && cli.installed_by_package)
            })
        };
        let program = |id: &str, cli: Option<&AgentCli>| {
            let program_path = cli.and_then(|cli| self.resolver.resolve(&cli.binary));
            AgentItem {
                id: id.to_owned(),
                detected: program_path.is_some(),
                program_path,
                path_source: cli.is_some().then(|| self.resolver.search().source),
                cli_installed_by_package: installed_by_package(id),
                cli: cli.map(CliSpec::of),
            }
        };
        let agents: Vec<AgentItem> = manifest
            .contributes
            .agent_adapters
            .iter()
            .map(|adapter| program(&adapter.id, adapter.cli.as_ref()))
            .collect();
        let programs: Vec<AgentItem> = manifest
            .contributes
            .host_programs
            .iter()
            .map(|host| program(&host.id, Some(&host.cli)))
            .collect();
        let effects = uninstall_effects(
            manifest,
            agents
                .iter()
                .chain(&programs)
                .any(|agent| agent.cli_installed_by_package),
        );
        Item {
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            description: manifest.description.clone(),
            version: manifest.version.to_string(),
            publisher: manifest.publisher.clone(),
            state: state_of(entry),
            installed: entry.map(installed_info),
            listing: ListingView::of(manifest),
            permissions: manifest
                .permissions
                .iter()
                .map(PermissionView::of)
                .collect(),
            risk: RiskView::of(&risk::assess(manifest)),
            readme: manifest.readme.clone(),
            detected: (!agents.is_empty() || !programs.is_empty())
                .then(|| agents.iter().chain(&programs).all(|agent| agent.detected)),
            agents,
            programs,
            settings: manifest
                .contributes
                .settings
                .iter()
                .map(|setting| SettingItem {
                    key: setting.key.clone(),
                    title: setting.title.clone(),
                    value_type: match setting.value_type {
                        SettingType::Boolean => "boolean",
                        SettingType::Integer => "integer",
                        SettingType::Number => "number",
                        SettingType::String => "string",
                    },
                    section: match setting.section {
                        SettingSection::Basic => "basic",
                        SettingSection::Advanced => "advanced",
                    },
                    default: setting.default.clone(),
                })
                .collect(),
            uninstall: UninstallItem {
                package_files: effects.package_files,
                storage: effects.storage,
                settings: effects.settings,
                secret_names: effects.secret_names,
                secrets_unscoped: effects.secrets_unscoped,
                connections: effects.connections,
                cli_binaries: effects.cli_binaries,
                offered_agent_homes: effects
                    .offered
                    .into_iter()
                    .map(|offer| match offer {
                        Offer::AgentCredentialHome { agent } => agent,
                    })
                    .collect(),
            },
            problem: None,
        }
    }
}

fn state_of(entry: Option<&LockEntry>) -> ItemState {
    match entry {
        None => ItemState::Available,
        Some(entry) if entry.enabled => ItemState::Enabled,
        Some(_) => ItemState::Disabled,
    }
}

fn installed_info(entry: &LockEntry) -> InstalledInfo {
    InstalledInfo {
        version: entry.version.clone(),
        source: entry.source.clone(),
        sha256: entry.sha256.clone(),
        installed_at_ms: entry.installed_at_ms,
        cli: entry.cli.clone(),
    }
}

/// An installed package whose files cannot be trusted: its lockfile record, the problem, and no manifest data. Its
/// files are still removable, so the danger zone keeps the always-true effects.
fn lockfile_item(entry: &LockEntry, error: &Error) -> Item {
    Item {
        id: entry.id.clone(),
        name: String::new(),
        description: String::new(),
        version: entry.version.clone(),
        publisher: String::new(),
        state: state_of(Some(entry)),
        installed: Some(installed_info(entry)),
        listing: ListingView {
            logo: None,
            category: "",
            developer: None,
            mention: None,
            banner: None,
            connections: Vec::new(),
            website: None,
            privacy: None,
            terms: None,
        },
        permissions: Vec::new(),
        risk: RiskView {
            level: "unknown",
            reasons: Vec::new(),
        },
        readme: None,
        agents: Vec::new(),
        programs: Vec::new(),
        detected: None,
        settings: Vec::new(),
        uninstall: UninstallItem {
            package_files: true,
            storage: true,
            settings: Vec::new(),
            secret_names: Vec::new(),
            secrets_unscoped: false,
            connections: Vec::new(),
            cli_binaries: Vec::new(),
            offered_agent_homes: Vec::new(),
        },
        problem: Some(ItemProblem {
            code: error.code().to_owned(),
            message: error.to_string(),
        }),
    }
}
