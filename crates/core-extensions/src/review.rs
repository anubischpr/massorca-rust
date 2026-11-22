//! The install review: what the package is, its risk, its listing, the CLI plan for this host, and the consent
//! fingerprint the install must send back.

use std::path::PathBuf;

use massorca_ext_package::manifest::Manifest;
use massorca_ext_package::manifest::agent::{AgentCli, CliOs};
use massorca_ext_package::risk;
use serde::Serialize;
use serde_json::{Value, json};

use crate::host::ProgramResolver;
use crate::lockfile::LockSource;
use crate::source::{LoadedPackage, sha256_hex};
use crate::views::{ListingView, PermissionView, RiskView};

/// Everything the person reviews before an install. Nothing was changed to build it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Review {
    pub id: String,
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub description: String,
    pub source: LockSource,
    pub sha256: String,
    pub permissions: Vec<PermissionView>,
    pub risk: RiskView,
    pub listing: ListingView,
    pub cli: Vec<CliPlan>,
    /// True when accepting the review also needs `run_installer`.
    pub runs_installer: bool,
    /// The fingerprint to send back as `consent`.
    pub consent: String,
}

/// What the install does about one agent adapter's CLI on this host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliPlan {
    pub agent: String,
    /// `None` when the adapter declares no `cli` block.
    pub binary: Option<String>,
    #[serde(flatten)]
    pub state: PlanState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PlanState {
    /// The program already resolves: the install only registers it.
    Detected { path: PathBuf },
    /// The vendor's command for this OS runs, exactly this argv.
    WillRun { command: Vec<String> },
    /// No `cli` block, or no command for this OS: the package registers, and the CLI stays missing.
    Unsupported,
}

/// The CLI plan of every agent adapter, in declaration order.
pub fn plan(
    manifest: &Manifest,
    resolver: &dyn ProgramResolver,
    os: Option<CliOs>,
) -> Vec<CliPlan> {
    manifest
        .contributes
        .agent_adapters
        .iter()
        .map(|adapter| {
            let cli = adapter.cli.as_ref();
            CliPlan {
                agent: adapter.id.clone(),
                binary: cli.map(|cli| cli.binary.clone()),
                state: cli.map_or(PlanState::Unsupported, |cli| plan_one(cli, resolver, os)),
            }
        })
        .collect()
}

fn plan_one(cli: &AgentCli, resolver: &dyn ProgramResolver, os: Option<CliOs>) -> PlanState {
    if let Some(path) = resolver.resolve(&cli.binary) {
        return PlanState::Detected { path };
    }
    cli.install
        .iter()
        .find(|step| Some(step.os) == os)
        .map_or(PlanState::Unsupported, |step| PlanState::WillRun {
            command: step.command.clone(),
        })
}

/// The consent fingerprint: SHA-256 over the id, the version, the package hash, the sorted permissions and the
/// planned installer commands. Encoded as a JSON array so it never depends on map ordering.
pub fn fingerprint(manifest: &Manifest, sha256: &str, plan: &[CliPlan]) -> String {
    let mut permissions: Vec<Value> = manifest
        .permissions
        .iter()
        .map(|permission| {
            let view = PermissionView::of(permission);
            let scope = view.scope.map(|mut scope| {
                scope.sort();
                scope
            });
            json!([view.kind, scope])
        })
        .collect();
    permissions.sort_by_key(Value::to_string);
    let installers: Vec<Value> = plan
        .iter()
        .filter_map(|entry| match &entry.state {
            PlanState::WillRun { command } => Some(json!([entry.agent, command])),
            _ => None,
        })
        .collect();
    let document = json!([
        "massorca.consent/1",
        manifest.id,
        manifest.version.to_string(),
        sha256.to_ascii_lowercase(),
        permissions,
        installers
    ]);
    sha256_hex(document.to_string().as_bytes())
}

/// Builds the review of a loaded package for this host.
pub fn review(
    package: &LoadedPackage,
    resolver: &dyn ProgramResolver,
    os: Option<CliOs>,
) -> Review {
    let manifest = &package.manifest;
    let cli = plan(manifest, resolver, os);
    let consent = fingerprint(manifest, &package.sha256, &cli);
    Review {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.to_string(),
        publisher: manifest.publisher.clone(),
        description: manifest.description.clone(),
        source: package.source.clone(),
        sha256: package.sha256.clone(),
        permissions: manifest
            .permissions
            .iter()
            .map(PermissionView::of)
            .collect(),
        risk: RiskView::of(&risk::assess(manifest)),
        listing: ListingView::of(manifest),
        runs_installer: cli
            .iter()
            .any(|entry| matches!(entry.state, PlanState::WillRun { .. })),
        cli,
        consent,
    }
}
