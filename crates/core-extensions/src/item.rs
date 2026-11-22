//! The data of the nine-part Marketplace item page, for an installed package or a catalogue entry, and the listed
//! images it shows. There is no item-specific code: everything comes from the manifest, the listing and the state.

use std::fs;
use std::path::PathBuf;

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
    /// For an agent package: whether every agent's CLI resolves on this host.
    pub detected: Option<bool>,
    /// Settings with their section (`basic` or `advanced`).
    pub settings: Vec<SettingItem>,
    /// The Danger Zone.
    pub uninstall: UninstallItem,
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
                Ok(self.item(&manifest::load(bytes)?, lockfile.get(entry.id)))
            })
            .collect()
    }

    /// Every installed package, in lockfile order.
    pub fn list(&self) -> Result<Vec<Item>, Error> {
        let lockfile = self.read_lockfile()?;
        lockfile
            .packages
            .iter()
            .map(|entry| Ok(self.item(&installed_manifest(&self.layout, entry)?, Some(entry))))
            .collect()
    }

    /// One item, installed or from the catalogue.
    pub fn get(&self, id: &str) -> Result<Item, Error> {
        let lockfile = self.read_lockfile()?;
        if let Some(entry) = lockfile.get(id) {
            return Ok(self.item(&installed_manifest(&self.layout, entry)?, Some(entry)));
        }
        let bytes = catalogue::find(id)
            .and_then(|entry| entry.file(manifest::MANIFEST_FILE))
            .ok_or_else(|| Error::NotFound(id.to_owned()))?;
        Ok(self.item(&manifest::load(bytes)?, None))
    }

    /// A listed image (logo or banner) of an item, at most [`MAX_ASSET_BYTES`].
    pub fn asset(&self, id: &str, path: &str) -> Result<Vec<u8>, Error> {
        let item = self.get(id)?;
        if !item.listing.images().any(|image| image == path) {
            return Err(Error::AssetNotListed(path.to_owned()));
        }
        let too_large = |size: u64| Error::AssetTooLarge {
            path: path.to_owned(),
            size,
            limit: MAX_ASSET_BYTES,
        };
        let lockfile = self.read_lockfile()?;
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

    fn item(&self, manifest: &Manifest, entry: Option<&LockEntry>) -> Item {
        let installed_by_package = |agent: &str| {
            entry.is_some_and(|entry| {
                entry
                    .cli
                    .iter()
                    .any(|cli| cli.agent == agent && cli.installed_by_package)
            })
        };
        let agents: Vec<AgentItem> = manifest
            .contributes
            .agent_adapters
            .iter()
            .map(|adapter| {
                let program_path = adapter
                    .cli
                    .as_ref()
                    .and_then(|cli| self.resolver.resolve(&cli.binary));
                AgentItem {
                    id: adapter.id.clone(),
                    detected: program_path.is_some(),
                    program_path,
                    cli_installed_by_package: installed_by_package(&adapter.id),
                    cli: adapter.cli.as_ref().map(CliSpec::of),
                }
            })
            .collect();
        let effects = uninstall_effects(
            manifest,
            agents.iter().any(|agent| agent.cli_installed_by_package),
        );
        Item {
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            description: manifest.description.clone(),
            version: manifest.version.to_string(),
            publisher: manifest.publisher.clone(),
            state: match entry {
                None => ItemState::Available,
                Some(entry) if entry.enabled => ItemState::Enabled,
                Some(_) => ItemState::Disabled,
            },
            installed: entry.map(|entry| InstalledInfo {
                version: entry.version.clone(),
                source: entry.source.clone(),
                sha256: entry.sha256.clone(),
                installed_at_ms: entry.installed_at_ms,
                cli: entry.cli.clone(),
            }),
            listing: ListingView::of(manifest),
            permissions: manifest
                .permissions
                .iter()
                .map(PermissionView::of)
                .collect(),
            risk: RiskView::of(&risk::assess(manifest)),
            readme: manifest.readme.clone(),
            detected: (!agents.is_empty()).then(|| agents.iter().all(|agent| agent.detected)),
            agents,
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
        }
    }
}
