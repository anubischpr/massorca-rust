//! `skills.bundled.list`, `skills.bundled.install` and `skills.bundled.refresh` (capability `skills-bundled/1`, SKL-04): the
//! skills MassOrca ships, placed for the chosen agents through the placement of SKL-02 and kept at this build's text. The list
//! only reads; the writes need host administration and run one at a time with the other placements. Removal is `skills.unplace`.

use std::path::PathBuf;
use std::sync::{Arc, PoisonError};

use massorca_core_extensions::skills::{
    self as core, BundledSkill, BundledState, BundledStatus, PlacementAgent as CoreAgent,
};
use massorca_protocol::methods::skill_bundled::{self as wire};
use massorca_protocol::methods::skill_placement::SkillPlacementResult;
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::skill_placement::{core_agents, record, report_to_wire, run, wire_agent};
use crate::features::skills::discovery_request;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

/// The folder a bundled skill is written to before it is placed: beside the placement record in the profile.
const STAGING: [&str; 2] = ["skills", "bundled"];

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let cap = capabilities::SKILLS_BUNDLED;
    Ok(vec![
        registries.capabilities.register(cap, Owner::Core)?,
        registries
            .methods
            .register(Owner::Core, wire::LIST, cap, Permission::Read, list)?,
        registries.methods.register(
            Owner::Core,
            wire::INSTALL,
            cap,
            Permission::HostAdmin,
            install,
        )?,
        registries.methods.register(
            Owner::Core,
            wire::REFRESH,
            cap,
            Permission::HostAdmin,
            refresh,
        )?,
    ])
}

async fn list(
    ctx: RequestCtx,
    _params: wire::SkillsBundledListParams,
) -> Result<wire::SkillsBundledListResult, ErrorBody> {
    let store = record(&ctx);
    let statuses = run(move || core::bundled_statuses(&store)).await?;
    Ok(wire::SkillsBundledListResult {
        skills: statuses.iter().map(bundled_to_wire).collect(),
    })
}

async fn install(
    ctx: RequestCtx,
    params: wire::SkillsBundledInstallParams,
) -> Result<SkillPlacementResult, ErrorBody> {
    let skill = bundled(&params.name)?;
    let agents = if params.agents.is_empty() {
        enabled_agents(&ctx).await?
    } else {
        core_agents(&params.agents)?
    };
    if agents.is_empty() {
        return Err(ErrorBody::new(
            codes::REQUEST_INVALID_PARAMS,
            "no agent tool is installed on this host; choose the agents to place it for",
        ));
    }
    let home = ctx.state.skills.home()?;
    let store = record(&ctx);
    let staging = staging(&ctx);
    let lock = Arc::clone(&ctx.state.skills.placements);
    let report = run(move || {
        let _held = lock.lock().unwrap_or_else(PoisonError::into_inner);
        core::install_bundled(&home, &store, &staging, skill, &agents)
    })
    .await?;
    Ok(report_to_wire(&report, true))
}

async fn refresh(
    ctx: RequestCtx,
    params: wire::SkillsBundledNameParams,
) -> Result<SkillPlacementResult, ErrorBody> {
    let skill = bundled(&params.name)?;
    let home = ctx.state.skills.home()?;
    let store = record(&ctx);
    let staging = staging(&ctx);
    let lock = Arc::clone(&ctx.state.skills.placements);
    let report = run(move || {
        let _held = lock.lock().unwrap_or_else(PoisonError::into_inner);
        core::refresh_bundled(&home, &store, &staging, skill)
    })
    .await?;
    Ok(report_to_wire(&report, true))
}

/// The bundled skill with this name, or the refusal for a name this build does not ship.
fn bundled(name: &str) -> Result<&'static BundledSkill, ErrorBody> {
    core::find_bundled(name).ok_or_else(|| {
        ErrorBody::new(
            codes::SKILL_NOT_FOUND,
            "MassOrca ships no skill with that name",
        )
    })
}

/// The agents whose skill root exists on this host: the ones an install places for when none is named.
async fn enabled_agents(ctx: &RequestCtx) -> Result<Vec<CoreAgent>, ErrorBody> {
    let request = discovery_request(ctx, Vec::new()).await?;
    let scan = ctx.state.skills.scan(request, true).await?;
    Ok(core::detected_agents(&scan.sources))
}

fn staging(ctx: &RequestCtx) -> PathBuf {
    ctx.state.profile.dir().join(STAGING[0]).join(STAGING[1])
}

fn bundled_to_wire(status: &BundledStatus) -> wire::BundledSkill {
    wire::BundledSkill {
        name: status.skill.name.to_owned(),
        description: status.skill.description().to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        state: match status.state {
            BundledState::NotPlaced => wire::BundledSkillState::NotPlaced,
            BundledState::Current => wire::BundledSkillState::Current,
            BundledState::Outdated => wire::BundledSkillState::Outdated,
        },
        agents: status
            .agents
            .iter()
            .map(|agent| wire_agent(*agent))
            .collect(),
    }
}
