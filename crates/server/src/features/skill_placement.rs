//! `skills.place-preview`, `skills.place`, `skills.unplace` and `skills.managed` (capability `skills-place/1`, SKL-02): copy a
//! home skill that discovery found into the folders of the chosen agents, and remove only the copies MassOrca placed and
//! left unchanged. The preview and the list only read; the two writes need host administration and run one at a time.

use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError};

use massorca_core_extensions::skills::{self as core, PlacementError, PlacementStore};
use massorca_protocol::methods::skill_placement::{self as wire};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::skills::discovery_request;
use crate::owner::Owner;
use crate::permission::{Permission, authorize};
use crate::registry::RegistrationHandle;

/// The placement record: a file in the profile, beside its other state.
const RECORD: [&str; 2] = ["skills", "placements.json"];

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let cap = capabilities::SKILLS_PLACE;
    Ok(vec![
        registries.capabilities.register(cap, Owner::Core)?,
        registries
            .methods
            .register(Owner::Core, wire::PREVIEW, cap, Permission::Read, preview)?,
        registries
            .methods
            .register(Owner::Core, wire::PLACE, cap, Permission::HostAdmin, place)?,
        registries.methods.register(
            Owner::Core,
            wire::UNPLACE,
            cap,
            Permission::HostAdmin,
            unplace,
        )?,
        registries
            .methods
            .register(Owner::Core, wire::MANAGED, cap, Permission::Read, managed)?,
    ])
}

async fn preview(
    ctx: RequestCtx,
    params: wire::SkillPlaceParams,
) -> Result<wire::SkillPlacementResult, ErrorBody> {
    let show_paths = authorize(&ctx.principal, Permission::HostAdmin).is_ok();
    let target = resolve(&ctx, &params, false).await?;
    let store = record(&ctx);
    let preview =
        run(move || core::preview_placement(&target.home, &store, &target.source, &target.agents))
            .await?;
    Ok(preview_to_wire(&preview, show_paths))
}

async fn place(
    ctx: RequestCtx,
    params: wire::SkillPlaceParams,
) -> Result<wire::SkillPlacementResult, ErrorBody> {
    let target = resolve(&ctx, &params, true).await?;
    let store = record(&ctx);
    let lock = Arc::clone(&ctx.state.skills.placements);
    let report = run(move || {
        let _held = lock.lock().unwrap_or_else(PoisonError::into_inner);
        core::place_skill(&target.home, &store, &target.source, &target.agents)
    })
    .await?;
    Ok(report_to_wire(&report, true))
}

async fn unplace(
    ctx: RequestCtx,
    params: wire::SkillUnplaceParams,
) -> Result<wire::SkillPlacementResult, ErrorBody> {
    let home = ctx.state.skills.home()?;
    let agents = core_agents(&params.agents)?;
    let store = record(&ctx);
    let lock = Arc::clone(&ctx.state.skills.placements);
    let name = params.name;
    let report = run(move || {
        let _held = lock.lock().unwrap_or_else(PoisonError::into_inner);
        core::unplace_skill(&home, &store, &name, &agents)
    })
    .await?;
    Ok(report_to_wire(&report, true))
}

async fn managed(
    ctx: RequestCtx,
    _params: wire::SkillsManagedParams,
) -> Result<wire::SkillsManagedResult, ErrorBody> {
    let show_paths = authorize(&ctx.principal, Permission::HostAdmin).is_ok();
    let store = record(&ctx);
    let skills = run(move || core::managed_placements(&store)).await?;
    Ok(wire::SkillsManagedResult {
        skills: skills
            .iter()
            .map(|skill| managed_to_wire(skill, show_paths))
            .collect(),
    })
}

/// What a placement works on: the home folder, the skill's folder, and the agents chosen.
struct Target {
    home: PathBuf,
    source: PathBuf,
    agents: Vec<core::PlacementAgent>,
}

/// Finds the skill the request names among those discovery lists on this host. Only a skill in an agent tool's own home
/// folder is placed: bundled, repository and plugin skills are not copied. Only a placement forces a fresh scan; a
/// preview, which any reader may ask for, reuses the shared one.
async fn resolve(
    ctx: &RequestCtx,
    params: &wire::SkillPlaceParams,
    refresh: bool,
) -> Result<Target, ErrorBody> {
    let agents = core_agents(&params.agents)?;
    let home = ctx.state.skills.home()?;
    let request = discovery_request(ctx, Vec::new()).await?;
    let scan = ctx.state.skills.scan(request, refresh).await?;
    let skill = scan
        .skills
        .iter()
        .find(|skill| skill.id == params.skill_id)
        .ok_or_else(|| {
            ErrorBody::new(
                codes::SKILL_NOT_FOUND,
                "no skill with that id is found on this host",
            )
        })?;
    if skill.source_kind != core::SkillSourceKind::Home {
        return Err(ErrorBody::new(
            codes::SKILL_NOT_PLACEABLE,
            "only a skill in an agent tool's home folder can be placed",
        ));
    }
    Ok(Target {
        home,
        source: skill.directory_path.clone(),
        agents,
    })
}

pub(crate) fn record(ctx: &RequestCtx) -> PlacementStore {
    PlacementStore::new(ctx.state.profile.dir().join(RECORD[0]).join(RECORD[1]))
}

/// Runs one placement call on the blocking pool and maps its refusal to the protocol error.
pub(crate) async fn run<T, F>(call: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, PlacementError> + Send + 'static,
{
    match tokio::task::spawn_blocking(call).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(placement_error(error)),
        // Why: a panic's text can name a host path, and a reader without host administration may get this.
        Err(_) => Err(ErrorBody::new(codes::INTERNAL, "the placement task failed")),
    }
}

/// The refusal as the protocol carries it. Messages name no host path, since a reader without host administration
/// may get one.
fn placement_error(error: PlacementError) -> ErrorBody {
    match error {
        PlacementError::InvalidName(name) => ErrorBody::new(
            codes::REQUEST_INVALID_PARAMS,
            format!("{name:?} is not a skill name MassOrca places"),
        ),
        PlacementError::NoAgents => {
            ErrorBody::new(codes::REQUEST_INVALID_PARAMS, "choose at least one agent")
        }
        PlacementError::NameTaken(name, _) => ErrorBody::new(
            codes::SKILL_NAME_TAKEN,
            format!(
                "a skill named {name} is already in an agent folder, and MassOrca did not place it"
            ),
        ),
        PlacementError::ContentChanged(name) => ErrorBody::new(
            codes::SKILL_CONTENT_CHANGED,
            format!("{name} changed since MassOrca placed it; unplace it first"),
        ),
        PlacementError::NotPlaced(name) => ErrorBody::new(
            codes::SKILL_NOT_PLACED,
            format!("MassOrca did not place {name}"),
        ),
        PlacementError::Unsupported { reason, .. } => ErrorBody::new(
            codes::SKILL_NOT_PLACEABLE,
            format!("the skill cannot be placed: {reason}"),
        ),
        PlacementError::InvalidRecord(_) => ErrorBody::new(
            codes::SKILL_RECORD_INVALID,
            "the placement record on this host cannot be read",
        ),
        PlacementError::Io { message, .. } => ErrorBody::new(codes::INTERNAL, message),
        PlacementError::Store(_) => {
            ErrorBody::new(codes::INTERNAL, "the placement record could not be saved")
        }
    }
}

pub(crate) fn core_agents(
    agents: &[wire::PlacementAgent],
) -> Result<Vec<core::PlacementAgent>, ErrorBody> {
    agents
        .iter()
        .map(|agent| {
            Ok(match agent {
                wire::PlacementAgent::Codex => core::PlacementAgent::Codex,
                wire::PlacementAgent::Claude => core::PlacementAgent::Claude,
                wire::PlacementAgent::Cursor => core::PlacementAgent::Cursor,
                wire::PlacementAgent::Gemini => core::PlacementAgent::Gemini,
                wire::PlacementAgent::Droid => core::PlacementAgent::Droid,
                wire::PlacementAgent::Continue => core::PlacementAgent::Continue,
                wire::PlacementAgent::Trae => core::PlacementAgent::Trae,
                wire::PlacementAgent::Grok => core::PlacementAgent::Grok,
                wire::PlacementAgent::Aug => core::PlacementAgent::Aug,
                wire::PlacementAgent::Muse => core::PlacementAgent::Muse,
                wire::PlacementAgent::Zcode => core::PlacementAgent::Zcode,
                wire::PlacementAgent::Unknown => {
                    return Err(ErrorBody::new(
                        codes::REQUEST_INVALID_PARAMS,
                        "`agents` names an agent this build does not know",
                    ));
                }
            })
        })
        .collect()
}

pub(crate) fn wire_agent(agent: core::PlacementAgent) -> wire::PlacementAgent {
    match agent {
        core::PlacementAgent::Codex => wire::PlacementAgent::Codex,
        core::PlacementAgent::Claude => wire::PlacementAgent::Claude,
        core::PlacementAgent::Cursor => wire::PlacementAgent::Cursor,
        core::PlacementAgent::Gemini => wire::PlacementAgent::Gemini,
        core::PlacementAgent::Droid => wire::PlacementAgent::Droid,
        core::PlacementAgent::Continue => wire::PlacementAgent::Continue,
        core::PlacementAgent::Trae => wire::PlacementAgent::Trae,
        core::PlacementAgent::Grok => wire::PlacementAgent::Grok,
        core::PlacementAgent::Aug => wire::PlacementAgent::Aug,
        core::PlacementAgent::Muse => wire::PlacementAgent::Muse,
        core::PlacementAgent::Zcode => wire::PlacementAgent::Zcode,
    }
}

fn skip_state(cause: &core::SkipCause) -> wire::PlacementState {
    match cause {
        core::SkipCause::Existing => wire::PlacementState::SkippedExisting,
        core::SkipCause::Modified => wire::PlacementState::SkippedModified,
        core::SkipCause::CopyFailed => wire::PlacementState::SkippedCopyFailed,
    }
}

fn outcome_state(outcome: &core::Outcome) -> wire::PlacementState {
    match outcome {
        core::Outcome::Canonical => wire::PlacementState::Canonical,
        core::Outcome::Placed(_) => wire::PlacementState::Placed,
        core::Outcome::AlreadyPlaced(_) => wire::PlacementState::AlreadyPlaced,
        core::Outcome::Removed => wire::PlacementState::Removed,
        core::Outcome::Skipped(cause) => skip_state(cause),
    }
}

fn plan_state(plan: &core::Plan) -> wire::PlacementState {
    match plan {
        core::Plan::Canonical => wire::PlacementState::Canonical,
        core::Plan::Create => wire::PlacementState::Create,
        core::Plan::AlreadyPlaced(_) => wire::PlacementState::AlreadyPlaced,
        core::Plan::Skipped(cause) => skip_state(cause),
    }
}

/// A host path, when the caller may see host paths.
fn path_text(path: &Path, show_paths: bool) -> Option<String> {
    show_paths.then(|| path.to_string_lossy().into_owned())
}

pub(crate) fn report_to_wire(
    report: &core::PlacementReport,
    show_paths: bool,
) -> wire::SkillPlacementResult {
    wire::SkillPlacementResult {
        name: report.name.clone(),
        canonical_path: path_text(&report.canonical, show_paths),
        agents: report
            .agents
            .iter()
            .map(|row| wire::AgentPlacement {
                agent: wire_agent(row.agent),
                state: outcome_state(&row.outcome),
                path: row
                    .path
                    .as_deref()
                    .and_then(|path| path_text(path, show_paths)),
                canonical: row.path.is_none(),
                topology: outcome_topology(&row.outcome),
            })
            .collect(),
    }
}

fn preview_to_wire(
    preview: &core::PlacementPreview,
    show_paths: bool,
) -> wire::SkillPlacementResult {
    wire::SkillPlacementResult {
        name: preview.name.clone(),
        canonical_path: path_text(&preview.canonical, show_paths),
        agents: preview
            .agents
            .iter()
            .map(|row| wire::AgentPlacement {
                agent: wire_agent(row.agent),
                state: plan_state(&row.plan),
                path: row
                    .path
                    .as_deref()
                    .and_then(|path| path_text(path, show_paths)),
                canonical: row.path.is_none(),
                topology: plan_topology(&row.plan),
            })
            .collect(),
    }
}

fn managed_to_wire(skill: &core::ManagedSkill, show_paths: bool) -> wire::ManagedSkill {
    wire::ManagedSkill {
        name: skill.name.clone(),
        canonical_path: path_text(&skill.canonical, show_paths),
        agents: skill
            .agents
            .iter()
            .map(|row| wire::ManagedAgent {
                agent: wire_agent(row.agent),
                path: row
                    .path
                    .as_deref()
                    .and_then(|path| path_text(path, show_paths)),
                canonical: row.path.is_none(),
                topology: Some(wire_topology(row.topology)),
            })
            .collect(),
    }
}

fn wire_topology(topology: core::Topology) -> wire::PlacementTopology {
    match topology {
        core::Topology::Canonical => wire::PlacementTopology::Canonical,
        core::Topology::Alias => wire::PlacementTopology::Alias,
        core::Topology::Copy => wire::PlacementTopology::Copy,
    }
}

/// The topology of a row that made or kept a folder; a removal or a skipped folder has none.
fn outcome_topology(outcome: &core::Outcome) -> Option<wire::PlacementTopology> {
    match outcome {
        core::Outcome::Canonical => Some(wire::PlacementTopology::Canonical),
        core::Outcome::Placed(topology) | core::Outcome::AlreadyPlaced(topology) => {
            Some(wire_topology(*topology))
        }
        core::Outcome::Removed | core::Outcome::Skipped(_) => None,
    }
}

/// The topology a preview shows: a folder not yet made has none until it is placed.
fn plan_topology(plan: &core::Plan) -> Option<wire::PlacementTopology> {
    match plan {
        core::Plan::Canonical => Some(wire::PlacementTopology::Canonical),
        core::Plan::AlreadyPlaced(topology) => Some(wire_topology(*topology)),
        core::Plan::Create | core::Plan::Skipped(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_task_that_panics_answers_with_a_fixed_message() {
        let error = run(|| -> Result<(), PlacementError> { panic!("/home/someone/secret") })
            .await
            .expect_err("the task failed");

        assert_eq!(error.code, codes::INTERNAL);
        assert_eq!(error.message, "the placement task failed");
    }
}
