//! `skills.discover` (capability `skills/1`, SKL-01): the skills agent tools and the registered projects keep on this
//! host. The scan is `core-extensions`' discovery and runs on the blocking pool; a scan is shared for ten seconds.
//! Read only. The paths of this host reach a host administrator only.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use massorca_core_extensions::skills::{self as core, DiscoveryRequest};
use massorca_protocol::methods::skills::{self as wire, SkillsDiscoverParams};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::workspace_domain::{blocking, shared};
use crate::owner::Owner;
use crate::permission::{Permission, authorize};
use crate::registry::RegistrationHandle;
use crate::service::DiscoveryOptions;

/// How long a scan is shared: a caller within it gets the last scan made for the same request (pv96 shares ten seconds).
const SCAN_TTL: Duration = Duration::from_secs(10);

/// What skill discovery reads on this host, and the last scan made for it.
#[derive(Debug)]
pub(crate) struct SkillScans {
    options: DiscoveryOptions,
    last: Mutex<Option<Shared>>,
}

#[derive(Debug)]
struct Shared {
    at: Instant,
    request: DiscoveryRequest,
    scan: core::SkillDiscovery,
}

impl SkillScans {
    pub(crate) fn new(options: DiscoveryOptions) -> Self {
        Self {
            options,
            last: Mutex::new(None),
        }
    }
}

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let cap = capabilities::SKILLS;
    Ok(vec![
        registries.capabilities.register(cap, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            wire::DISCOVER,
            cap,
            Permission::Read,
            discover,
        )?,
    ])
}

async fn discover(
    ctx: RequestCtx,
    params: SkillsDiscoverParams,
) -> Result<wire::SkillDiscovery, ErrorBody> {
    let source_kinds = params
        .source_kinds
        .iter()
        .map(core_kind)
        .collect::<Result<Vec<_>, _>>()?;
    let domain = shared(&ctx);
    let projects = blocking(move || domain.list_projects()).await?;
    let scans = &ctx.state.skills;
    let home = scans.options.home.clone().ok_or_else(|| {
        ErrorBody::new(codes::INTERNAL, "there is no home folder to scan skills in")
    })?;
    let request = DiscoveryRequest {
        home,
        folders: projects
            .iter()
            .map(|project| PathBuf::from(&project.path))
            .collect(),
        env: scans.options.env.clone(),
        source_kinds,
    };
    let scan = scan_now(scans, request, params.refresh).await?;
    let show_paths = authorize(&ctx.principal, Permission::HostAdmin).is_ok();
    Ok(to_wire(&scan, show_paths))
}

/// The shared scan when it is fresh and was made for `request`, otherwise a new scan that replaces it.
async fn scan_now(
    scans: &SkillScans,
    request: DiscoveryRequest,
    refresh: bool,
) -> Result<core::SkillDiscovery, ErrorBody> {
    {
        let last = scans.last.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(shared) = last.as_ref().filter(|shared| {
            !refresh && shared.request == request && shared.at.elapsed() < SCAN_TTL
        }) {
            return Ok(shared.scan.clone());
        }
    }
    let scanning = request.clone();
    let scan = tokio::task::spawn_blocking(move || core::discover(&scanning))
        .await
        .map_err(|error| {
            ErrorBody::new(codes::INTERNAL, format!("the skill scan failed: {error}"))
        })?;
    let mut last = scans.last.lock().unwrap_or_else(PoisonError::into_inner);
    *last = Some(Shared {
        at: Instant::now(),
        request,
        scan: scan.clone(),
    });
    Ok(scan)
}

fn core_kind(kind: &wire::SkillSourceKind) -> Result<core::SkillSourceKind, ErrorBody> {
    Ok(match kind {
        wire::SkillSourceKind::Home => core::SkillSourceKind::Home,
        wire::SkillSourceKind::Repo => core::SkillSourceKind::Repo,
        wire::SkillSourceKind::Bundled => core::SkillSourceKind::Bundled,
        wire::SkillSourceKind::Plugin => core::SkillSourceKind::Plugin,
        wire::SkillSourceKind::Unknown => {
            return Err(ErrorBody::new(
                codes::REQUEST_INVALID_PARAMS,
                "`source_kinds` names a kind this build does not know",
            ));
        }
    })
}

fn wire_kind(kind: core::SkillSourceKind) -> wire::SkillSourceKind {
    match kind {
        core::SkillSourceKind::Home => wire::SkillSourceKind::Home,
        core::SkillSourceKind::Repo => wire::SkillSourceKind::Repo,
        core::SkillSourceKind::Bundled => wire::SkillSourceKind::Bundled,
        core::SkillSourceKind::Plugin => wire::SkillSourceKind::Plugin,
    }
}

fn wire_provider(provider: &core::SkillProvider) -> wire::SkillProvider {
    match provider {
        core::SkillProvider::Codex => wire::SkillProvider::Codex,
        core::SkillProvider::Claude => wire::SkillProvider::Claude,
        core::SkillProvider::AgentSkills => wire::SkillProvider::AgentSkills,
    }
}

fn wire_reason(reason: core::SkipReason) -> wire::SkipReason {
    match reason {
        core::SkipReason::Missing => wire::SkipReason::Missing,
        core::SkipReason::Unreadable => wire::SkipReason::Unreadable,
        core::SkipReason::Truncated => wire::SkipReason::Truncated,
    }
}

/// The scan as the protocol carries it. Host paths are left out unless `show_paths` is set.
fn to_wire(scan: &core::SkillDiscovery, show_paths: bool) -> wire::SkillDiscovery {
    let path = |path: &Path| show_paths.then(|| path.to_string_lossy().into_owned());
    wire::SkillDiscovery {
        skills: scan
            .skills
            .iter()
            .map(|skill| wire::DiscoveredSkill {
                id: skill.id.clone(),
                name: skill.name.clone(),
                description: skill.description.clone(),
                providers: skill.providers.iter().map(wire_provider).collect(),
                source_kind: wire_kind(skill.source_kind),
                source_label: skill.source_label.clone(),
                root_path: path(&skill.root_path),
                root_paths: match show_paths {
                    true => skill
                        .root_paths
                        .iter()
                        .map(|root| root.to_string_lossy().into_owned())
                        .collect(),
                    false => Vec::new(),
                },
                directory_path: path(&skill.directory_path),
                skill_file_path: path(&skill.skill_file_path),
                installed: skill.installed,
                updated_at: skill.updated_at_ms,
                disable_slash_command: skill.disable_slash_command,
            })
            .collect(),
        sources: scan
            .sources
            .iter()
            .map(|source| wire::SkillSource {
                id: source.id.clone(),
                label: source.label.clone(),
                path: path(&source.path),
                source_kind: wire_kind(source.source_kind),
                providers: source.providers.iter().map(wire_provider).collect(),
                owner: source.owner.clone(),
                exists: source.exists,
                skipped_reason: source.skipped_reason.map(wire_reason),
            })
            .collect(),
        scanned_at: scan.scanned_at_ms,
    }
}
