//! Extension methods: install, list, enable, disable and remove packages (capability `extensions/1`, decision 0062).
//!
//! Every enum here has an `Unknown` variant for values from a newer peer; fields are only ever added (0002).

use serde::{Deserialize, Serialize};

/// Params: none. Result: [`ExtensionList`], installed packages in install order.
pub const LIST: &str = "extensions.list";
/// Params: [`ExtensionRef`]. Result: [`ExtensionItem`], installed or from the catalogue.
pub const GET: &str = "extensions.get";
/// Params: none. Result: [`ExtensionList`], the built-in catalogue with each entry's state.
pub const CATALOGUE: &str = "extensions.catalogue";
/// Params: [`ExtensionInspectParams`]. Result: [`ExtensionReview`]. Changes nothing.
pub const INSPECT: &str = "extensions.inspect";
/// Params: [`ExtensionAssetParams`]. Result: [`ExtensionAsset`], a listed image.
pub const ASSET: &str = "extensions.asset";
/// Params: [`ExtensionInstallParams`]. Result: [`ExtensionInstalled`]. Can run a vendor program on the host.
pub const INSTALL: &str = "extensions.install";
/// Params: [`ExtensionSetEnabledParams`]. Result: [`ExtensionEnabled`].
pub const SET_ENABLED: &str = "extensions.set_enabled";
/// Params: [`ExtensionRemoveParams`]. Result: [`ExtensionRemoved`]. Can run a vendor program on the host.
pub const REMOVE: &str = "extensions.remove";

/// Selector for `extensions.get`: a package id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionRef {
    pub id: String,
}

/// Params of `extensions.inspect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionInspectParams {
    /// `catalogue:<id>`, `dir:<path>` or `file:<path>`, read on the service host.
    pub source: String,
}

/// Params of `extensions.asset`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionAssetParams {
    pub id: String,
    /// A package path the item's listing names, such as its logo or banner image.
    pub path: String,
}

/// Result of `extensions.asset`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionAsset {
    pub id: String,
    pub path: String,
    pub size: u64,
    /// The file's bytes, base64.
    pub data: String,
}

/// Params of `extensions.install`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionInstallParams {
    pub source: String,
    /// The `consent` fingerprint of the review the person accepted. Without it the install is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent: Option<String>,
    /// Accepts running the vendor installer the review lists. Separate from `consent`.
    #[serde(default)]
    pub run_installer: bool,
    /// Pins the package. Required for a `file:` source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// Params of `extensions.set_enabled`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionSetEnabledParams {
    pub id: String,
    pub enabled: bool,
}

/// Params of `extensions.remove`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExtensionRemoveParams {
    pub id: String,
    /// Also runs the cleanups other features registered, such as deleting an agent's credential home.
    #[serde(default)]
    pub delete_agent_homes: bool,
    /// Also runs the uninstall command of each CLI this package installed.
    #[serde(default)]
    pub uninstall_cli: bool,
}

/// Result of `extensions.list` and `extensions.catalogue`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionList {
    pub extensions: Vec<ExtensionItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionState {
    Available,
    Enabled,
    Disabled,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// One Marketplace item: the data of the nine-part item page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub publisher: String,
    pub state: ExtensionState,
    #[serde(default)]
    pub installed: Option<InstalledInfo>,
    pub listing: Listing,
    #[serde(default)]
    pub permissions: Vec<Permission>,
    pub risk: Risk,
    #[serde(default)]
    pub readme: Option<String>,
    #[serde(default)]
    pub agents: Vec<AgentItem>,
    /// For an agent package: whether every agent's CLI resolves on the host.
    #[serde(default)]
    pub detected: Option<bool>,
    #[serde(default)]
    pub settings: Vec<SettingItem>,
    /// The Danger Zone.
    pub uninstall: UninstallEffects,
}

/// What the lockfile records about an installed package.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstalledInfo {
    pub version: String,
    pub source: SourceRef,
    pub sha256: String,
    pub installed_at_ms: u64,
    /// What the install did about each agent CLI, including the vendor command that ran.
    #[serde(default)]
    pub cli: Vec<CliRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Catalogue,
    Dir,
    File,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Where a package was installed from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    pub kind: SourceKind,
    #[serde(rename = "ref")]
    pub reference: String,
}

/// What an install did about one agent CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliRecord {
    pub agent: String,
    pub binary: String,
    pub detected_before_install: bool,
    #[serde(default)]
    pub installer: Option<InstallerRun>,
    pub installed_by_package: bool,
}

/// The vendor installer that ran: the exact argv, when, and its exit code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallerRun {
    pub command: Vec<String>,
    pub ran_at_ms: u64,
    #[serde(default)]
    pub exit_code: Option<i32>,
}

/// One declared permission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permission {
    pub kind: String,
    pub reason: String,
    /// The scope entries; `None` means "any".
    #[serde(default)]
    pub scope: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// The computed risk of a package, with the reasons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Risk {
    pub level: RiskLevel,
    #[serde(default)]
    pub reasons: Vec<RiskReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskReason {
    pub level: RiskLevel,
    pub subject: String,
    pub copy: String,
}

/// The Marketplace page data of a package's `listing`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listing {
    #[serde(default)]
    pub logo: Option<String>,
    /// `extensions`, `agents`, `skills`, `services` or `themes`; other values may come from a newer peer.
    pub category: String,
    #[serde(default)]
    pub developer: Option<Developer>,
    #[serde(default)]
    pub mention: Option<String>,
    #[serde(default)]
    pub banner: Option<Banner>,
    #[serde(default)]
    pub connections: Vec<Connection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Developer {
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Banner {
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub examples: Vec<BannerExample>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BannerExample {
    /// `mention`, `command` or `usage`.
    pub kind: String,
    #[serde(default)]
    pub text: Option<String>,
    pub caption: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    /// `oauth`, `api-key` or `agent-login`.
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub multiple: bool,
}

/// An agent of an agent package, with its CLI on this host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentItem {
    pub id: String,
    pub detected: bool,
    #[serde(default)]
    pub program_path: Option<String>,
    pub cli_installed_by_package: bool,
    #[serde(default)]
    pub cli: Option<CliSpec>,
}

/// An agent adapter's `cli` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliSpec {
    pub binary: String,
    pub verify: Vec<String>,
    pub source: String,
    #[serde(default)]
    pub install: Vec<CliStep>,
    #[serde(default)]
    pub uninstall: Vec<CliStep>,
}

/// One command for one operating system, as argv.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliStep {
    /// `windows`, `linux` or `macos`.
    pub os: String,
    pub command: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingItem {
    pub key: String,
    pub title: String,
    #[serde(rename = "type")]
    pub value_type: String,
    /// `basic` or `advanced`.
    pub section: String,
    pub default: serde_json::Value,
}

/// What removing the package affects: the Danger Zone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UninstallEffects {
    pub package_files: bool,
    pub storage: bool,
    #[serde(default)]
    pub settings: Vec<String>,
    #[serde(default)]
    pub secret_names: Vec<String>,
    #[serde(default)]
    pub secrets_unscoped: bool,
    #[serde(default)]
    pub connections: Vec<String>,
    #[serde(default)]
    pub cli_binaries: Vec<String>,
    /// Agent credential homes, deleted only when the person asks.
    #[serde(default)]
    pub offered_agent_homes: Vec<String>,
}

/// Result of `extensions.inspect`: everything the person reviews before an install.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionReview {
    pub id: String,
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub description: String,
    pub source: SourceRef,
    pub sha256: String,
    #[serde(default)]
    pub permissions: Vec<Permission>,
    pub risk: Risk,
    pub listing: Listing,
    #[serde(default)]
    pub cli: Vec<CliPlan>,
    /// Accepting the review also needs `run_installer`.
    pub runs_installer: bool,
    /// The fingerprint to send back as `consent`.
    pub consent: String,
}

/// What the install does about one agent adapter's CLI on the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliPlan {
    pub agent: String,
    #[serde(default)]
    pub binary: Option<String>,
    #[serde(flatten)]
    pub state: CliPlanState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CliPlanState {
    /// The program already resolves: the install only registers it.
    Detected { path: String },
    /// The vendor's command for the host OS runs, exactly this argv.
    WillRun { command: Vec<String> },
    /// No command fits: the package registers, and the CLI stays missing.
    Unsupported,
    /// A state from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Result of `extensions.install`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionInstalled {
    pub id: String,
    pub version: String,
    pub source: SourceRef,
    pub sha256: String,
    pub enabled: bool,
    pub installed_at_ms: u64,
    #[serde(default)]
    pub cli: Vec<CliRecord>,
}

/// Result of `extensions.set_enabled`. `changed` is false when the package was already in that state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionEnabled {
    pub id: String,
    pub enabled: bool,
    pub changed: bool,
}

/// Result of `extensions.remove`. The package is gone; anything in `failures` was not undone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionRemoved {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub failures: Vec<RemovalFailure>,
    #[serde(default)]
    pub cli: Vec<CliRemoval>,
}

/// A step after the removal commit that failed: a file deletion (owner `extensions`) or a registered cleanup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovalFailure {
    pub owner: String,
    pub code: String,
    pub message: String,
}

/// What removal did about one agent CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliRemoval {
    pub agent: String,
    pub binary: String,
    #[serde(flatten)]
    pub action: CliRemovalAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum CliRemovalAction {
    /// The package did not install this CLI, so it is left alone.
    LeftAlone,
    /// The package installed it, but declares no uninstall command for this OS.
    NoCommand,
    /// The uninstall command was not asked for; the Danger Zone can offer it.
    Offered { command: Vec<String> },
    /// The uninstall command ran and exited 0.
    Ran {
        command: Vec<String>,
        #[serde(default)]
        exit_code: Option<i32>,
    },
    /// The uninstall command ran and failed. Nothing is rolled back.
    Failed {
        command: Vec<String>,
        message: String,
    },
    /// An action from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn unknown_values_from_a_newer_peer_decode() {
        let plan: CliPlan =
            serde_json::from_value(json!({ "agent": "a", "state": "brand_new", "extra": 1 }))
                .expect("a plan from a newer peer");
        assert_eq!(plan.state, CliPlanState::Unknown);
        let state: ExtensionState = serde_json::from_value(json!("archived")).expect("a state");
        assert_eq!(state, ExtensionState::Unknown);
        let removal: CliRemoval =
            serde_json::from_value(json!({ "agent": "a", "binary": "a", "action": "later" }))
                .expect("a removal");
        assert_eq!(removal.action, CliRemovalAction::Unknown);
    }

    #[test]
    fn an_install_without_options_decodes_with_defaults() {
        let params: ExtensionInstallParams =
            serde_json::from_value(json!({ "source": "dir:/p" })).expect("params");
        assert_eq!(params.consent, None);
        assert!(!params.run_installer);
        let remove: ExtensionRemoveParams =
            serde_json::from_value(json!({ "id": "x.y" })).expect("params");
        assert!(!remove.delete_agent_homes && !remove.uninstall_cli);
    }
}
