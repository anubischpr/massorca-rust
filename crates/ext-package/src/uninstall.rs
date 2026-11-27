//! What removing an extension deletes, and what is only offered. Computed from the manifest, because a manifest cannot
//! declare it: it can only leave something out, and the computation never trusts it to say so.

use crate::manifest::Manifest;
use crate::permission::{PermissionKind, PermissionScope};

/// Everything removing the extension touches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallEffects {
    /// The package files under the installed-package directory. Always true.
    pub package_files: bool,
    /// The extension's stored data (`<profile>/extensions/data/<id>/`). Always true.
    pub storage: bool,
    /// Every declared setting key, in every section, with its stored value.
    pub settings: Vec<String>,
    /// Secret names removed, as scoped by the `secrets` permission.
    pub secret_names: Vec<String>,
    /// The `secrets` permission has no scope, so every secret the extension stored is removed. The caller reads its
    /// own store to enumerate them.
    pub secrets_unscoped: bool,
    /// Connection ids whose stored sign-in is removed.
    pub connections: Vec<String>,
    /// CLI binaries removed. Empty unless the package installed them (`cli_installed`).
    pub cli_binaries: Vec<String>,
    /// Things that are kept unless the user asks for them.
    pub offered: Vec<Offer>,
}

/// Data the removal leaves in place unless the user chooses to delete it too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offer {
    /// The credential home (0056) of an agent the package provides.
    AgentCredentialHome { agent: String },
}

/// Computes what removing `manifest` deletes. `cli_installed` says whether the install step registered the CLI
/// itself (the package installed it, so removal may offer to remove it).
pub fn uninstall_effects(manifest: &Manifest, cli_installed: bool) -> UninstallEffects {
    let contributes = &manifest.contributes;
    let secrets = manifest
        .permissions
        .iter()
        .find(|permission| permission.kind == PermissionKind::Secrets);
    let (secret_names, secrets_unscoped) =
        match secrets.and_then(|permission| permission.scope.as_ref()) {
            Some(PermissionScope::Secrets(names)) => (names.clone(), false),
            _ => (Vec::new(), secrets.is_some()),
        };
    let cli_binaries = if cli_installed {
        manifest
            .program_clis()
            .map(|(_, cli)| cli.binary.clone())
            .collect()
    } else {
        Vec::new()
    };
    let connections = manifest
        .listing
        .as_ref()
        .map(|listing| {
            listing
                .connections
                .iter()
                .map(|connection| connection.id.clone())
                .collect()
        })
        .unwrap_or_default();
    UninstallEffects {
        package_files: true,
        storage: true,
        settings: contributes
            .settings
            .iter()
            .map(|setting| setting.key.clone())
            .collect(),
        secret_names,
        secrets_unscoped,
        connections,
        cli_binaries,
        offered: contributes
            .agent_adapters
            .iter()
            .map(|adapter| Offer::AgentCredentialHome {
                agent: adapter.id.clone(),
            })
            .collect(),
    }
}
