//! Placing a skill for agent tools (SKL-02, as pv96 places it): one canonical copy at `.agents/skills/<name>`, then in each
//! chosen agent's folder an alias to that copy (a relative symlink on Unix, a junction on Windows), or a verified copy when
//! an alias is refused. Removal checks each placement by how it was made: an alias by its target, a copy by its digest. A
//! folder the user already has is never overwritten.

use std::fs;
use std::path::{Path, PathBuf};

use super::placement_agents::PlacementAgent;
use super::placement_error::PlacementError;
use super::placement_link::{create_link, link_points_to, remove_link};
use super::placement_model::{
    AgentOutcome, LinkMode, ManagedAgent, ManagedSkill, Outcome, PlacementPreview, PlacementReport,
    PlacementStore, Plan, PlannedAgent, SkipCause, Topology,
};
use super::placement_record::{load, save};
use super::placement_tree::{is_link, is_plain_folder, place_copy, sweep_staging, tree_digest};

/// What placing `source` would do, and the refusals that would stop it. Writes nothing.
pub fn preview_placement(
    home: &Path,
    store: &PlacementStore,
    source: &Path,
    agents: &[PlacementAgent],
) -> Result<PlacementPreview, PlacementError> {
    let prepared = prepare(home, store, source, agents)?;
    Ok(PlacementPreview {
        name: prepared.name,
        canonical: prepared.canonical,
        agents: prepared
            .slots
            .into_iter()
            .map(|slot| PlannedAgent {
                agent: slot.agent,
                path: slot.path,
                plan: slot.plan,
            })
            .collect(),
    })
}

/// Places `source` with links where the filesystem allows them, and copies elsewhere.
pub fn place_skill(
    home: &Path,
    store: &PlacementStore,
    source: &Path,
    agents: &[PlacementAgent],
) -> Result<PlacementReport, PlacementError> {
    place_skill_with(home, store, source, agents, LinkMode::Prefer)
}

/// Places `source` as the canonical copy and in each chosen agent's folder. A folder that cannot take a link is copied
/// and verified; one that cannot be placed is skipped and reported, and the canonical copy and the rest stand.
pub fn place_skill_with(
    home: &Path,
    store: &PlacementStore,
    source: &Path,
    agents: &[PlacementAgent],
    links: LinkMode,
) -> Result<PlacementReport, PlacementError> {
    let prepared = prepare(home, store, source, agents)?;
    for folder in skill_folders(home) {
        sweep_staging(&folder, &prepared.name);
    }
    if !prepared.canonical_managed {
        place_copy(source, &prepared.canonical)?;
    }
    let rows: Vec<AgentOutcome> = prepared
        .slots
        .iter()
        .map(|slot| AgentOutcome {
            agent: slot.agent,
            path: slot.path.clone(),
            outcome: act(slot, source, &prepared, links),
        })
        .collect();

    let mut skills = load(store)?;
    let previous = skills
        .iter()
        .position(|skill| skill.name == prepared.name)
        .map(|index| skills.remove(index))
        .map_or_else(Vec::new, |skill| skill.agents);
    let mut agents_now: Vec<ManagedAgent> = previous
        .into_iter()
        .filter(|kept| {
            rows.iter().any(|row| {
                row.agent == kept.agent && row.outcome == Outcome::Skipped(SkipCause::Modified)
            }) || !rows.iter().any(|row| row.agent == kept.agent)
        })
        .collect();
    for row in &rows {
        if let Some(topology) = placed_topology(&row.outcome) {
            agents_now.push(ManagedAgent {
                agent: row.agent,
                path: row.path.clone(),
                topology,
            });
        }
    }
    skills.push(ManagedSkill {
        name: prepared.name.clone(),
        canonical: prepared.canonical.clone(),
        digest: prepared.digest,
        agents: agents_now,
    });
    save(store, &skills)?;
    Ok(PlacementReport {
        name: prepared.name,
        canonical: prepared.canonical,
        agents: rows,
    })
}

/// Removes the MassOrca placements of `name` for `agents`; an empty list means every agent MassOrca placed for. A link is
/// removed as a link, never through to the canonical copy; a copy the user changed is kept and reported. The canonical copy
/// goes once no placement depends on it.
pub fn unplace_skill(
    home: &Path,
    store: &PlacementStore,
    name: &str,
    agents: &[PlacementAgent],
) -> Result<PlacementReport, PlacementError> {
    let mut skills = load(store)?;
    let index = skills
        .iter()
        .position(|skill| skill.name == name)
        .ok_or_else(|| PlacementError::NotPlaced(name.to_owned()))?;
    let mut skill = skills.remove(index);
    let canonical = canonical_path(home, name);
    for folder in skill_folders(home) {
        sweep_staging(&folder, name);
    }
    let wanted: Vec<PlacementAgent> = if agents.is_empty() {
        skill.agents.iter().map(|row| row.agent).collect()
    } else {
        agents.to_vec()
    };

    let mut rows = Vec::new();
    let mut kept = Vec::new();
    for row in std::mem::take(&mut skill.agents) {
        if !wanted.contains(&row.agent) {
            kept.push(row);
            continue;
        }
        let outcome = match row.path.as_deref() {
            None => Outcome::Removed,
            Some(path) => {
                match remove_placement(home, path, row.topology, &skill.digest, &canonical) {
                    Removal::Gone | Removal::Removed => Outcome::Removed,
                    Removal::Modified => Outcome::Skipped(SkipCause::Modified),
                    Removal::Failed => Outcome::Skipped(SkipCause::CopyFailed),
                }
            }
        };
        if matches!(
            outcome,
            Outcome::Skipped(SkipCause::Modified | SkipCause::CopyFailed)
        ) {
            kept.push(row.clone());
        }
        rows.push(AgentOutcome {
            agent: row.agent,
            path: row.path,
            outcome,
        });
    }

    if kept.is_empty() {
        // Why: the canonical copy goes only when nothing still reaches it, so no alias is left dangling.
        let canonical_gone = matches!(
            remove_copy(home, &canonical, &skill.digest),
            Removal::Gone | Removal::Removed
        );
        if !canonical_gone {
            skill.agents = kept;
            skills.push(skill);
        }
    } else {
        skill.agents = kept;
        skills.push(skill);
    }
    save(store, &skills)?;
    Ok(PlacementReport {
        name: name.to_owned(),
        canonical,
        agents: rows,
    })
}

/// The skills MassOrca placed, as recorded.
pub fn managed_placements(store: &PlacementStore) -> Result<Vec<ManagedSkill>, PlacementError> {
    load(store)
}

/// The checks and per-agent plan that placing `source` needs. Refusals come out here, before anything is written.
struct Prepared {
    name: String,
    digest: String,
    canonical: PathBuf,
    canonical_managed: bool,
    slots: Vec<Slot>,
}

struct Slot {
    agent: PlacementAgent,
    path: Option<PathBuf>,
    plan: Plan,
}

fn prepare(
    home: &Path,
    store: &PlacementStore,
    source: &Path,
    agents: &[PlacementAgent],
) -> Result<Prepared, PlacementError> {
    let mut wanted: Vec<PlacementAgent> = Vec::new();
    for agent in agents {
        if !wanted.contains(agent) {
            wanted.push(*agent);
        }
    }
    if wanted.is_empty() {
        return Err(PlacementError::NoAgents);
    }
    let name = source_name(source)?;
    let digest = tree_digest(source)?;
    let recorded = load(store)?.into_iter().find(|skill| skill.name == name);
    if recorded
        .as_ref()
        .is_some_and(|skill| skill.digest != digest)
    {
        return Err(PlacementError::ContentChanged(name));
    }

    let canonical = canonical_path(home, &name);
    let canonical_managed = if fs::symlink_metadata(&canonical).is_ok() {
        // A record whose digest equals `digest` (checked above) and a canonical copy that still matches it.
        let ours =
            recorded.is_some() && tree_digest(&canonical).ok().as_deref() == Some(digest.as_str());
        if !ours {
            return Err(PlacementError::NameTaken(name, canonical));
        }
        true
    } else {
        false
    };

    let slots = wanted
        .into_iter()
        .map(|agent| match agent_folder(home, agent, &name) {
            None => Slot {
                agent,
                path: None,
                plan: Plan::Canonical,
            },
            Some(path) => {
                let plan = agent_plan(&path, agent, recorded.as_ref(), &digest, &canonical);
                Slot {
                    agent,
                    path: Some(path),
                    plan,
                }
            }
        })
        .collect();
    Ok(Prepared {
        name,
        digest,
        canonical,
        canonical_managed,
        slots,
    })
}

/// What one agent's folder needs: nothing when it still holds the placement MassOrca made, a placement when it is
/// missing, and a skip when the user owns it or changed MassOrca's placement.
fn agent_plan(
    path: &Path,
    agent: PlacementAgent,
    recorded: Option<&ManagedSkill>,
    digest: &str,
    canonical: &Path,
) -> Plan {
    if fs::symlink_metadata(path).is_err() {
        return Plan::Create;
    }
    let row = recorded.and_then(|skill| {
        skill
            .agents
            .iter()
            .find(|row| row.agent == agent && row.path.as_deref() == Some(path))
    });
    let Some(row) = row else {
        return Plan::Skipped(SkipCause::Existing);
    };
    let intact = match row.topology {
        Topology::Alias => link_points_to(path, canonical),
        Topology::Copy | Topology::Canonical => tree_digest(path).ok().as_deref() == Some(digest),
    };
    if intact {
        Plan::AlreadyPlaced(row.topology)
    } else {
        Plan::Skipped(SkipCause::Modified)
    }
}

fn act(slot: &Slot, source: &Path, prepared: &Prepared, links: LinkMode) -> Outcome {
    match (&slot.plan, &slot.path) {
        (Plan::Create, Some(path)) => {
            place_one(path, source, &prepared.canonical, &prepared.digest, links)
        }
        (Plan::Create, None) | (Plan::Canonical, _) => Outcome::Canonical,
        (Plan::AlreadyPlaced(topology), _) => Outcome::AlreadyPlaced(*topology),
        (Plan::Skipped(cause), _) => Outcome::Skipped(cause.clone()),
    }
}

/// Places one missing folder: an alias when links are preferred and the filesystem takes one, else a copy that must
/// match the source's digest before it counts.
fn place_one(
    path: &Path,
    source: &Path,
    canonical: &Path,
    digest: &str,
    links: LinkMode,
) -> Outcome {
    if links == LinkMode::Prefer {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if create_link(canonical, path).is_ok() {
            return Outcome::Placed(Topology::Alias);
        }
    }
    match place_copy(source, path) {
        Ok(()) if tree_digest(path).ok().as_deref() == Some(digest) => {
            Outcome::Placed(Topology::Copy)
        }
        Ok(()) => {
            let _ = fs::remove_dir_all(path);
            Outcome::Skipped(SkipCause::CopyFailed)
        }
        Err(_) => Outcome::Skipped(SkipCause::CopyFailed),
    }
}

fn placed_topology(outcome: &Outcome) -> Option<Topology> {
    match outcome {
        Outcome::Canonical => Some(Topology::Canonical),
        Outcome::Placed(topology) | Outcome::AlreadyPlaced(topology) => Some(*topology),
        Outcome::Removed | Outcome::Skipped(_) => None,
    }
}

enum Removal {
    Gone,
    Removed,
    Modified,
    Failed,
}

/// Removes one placement by how it was made: an alias only while it still points at the canonical copy, and it is removed
/// as a link; a copy only while its digest still matches.
fn remove_placement(
    home: &Path,
    path: &Path,
    topology: Topology,
    digest: &str,
    canonical: &Path,
) -> Removal {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return Removal::Gone;
    };
    if !path.starts_with(home) {
        return Removal::Modified;
    }
    match topology {
        Topology::Alias => {
            if is_link(&meta) && link_points_to(path, canonical) {
                match remove_link(path) {
                    Ok(()) => Removal::Removed,
                    Err(_) => Removal::Failed,
                }
            } else {
                Removal::Modified
            }
        }
        Topology::Copy | Topology::Canonical => remove_copy(home, path, digest),
    }
}

/// Removes a copy when it is still a plain folder with the recorded digest, under `home`.
fn remove_copy(home: &Path, path: &Path, digest: &str) -> Removal {
    if fs::symlink_metadata(path).is_err() {
        return Removal::Gone;
    }
    if !path.starts_with(home)
        || !is_plain_folder(path)
        || tree_digest(path).ok().as_deref() != Some(digest)
    {
        return Removal::Modified;
    }
    match fs::remove_dir_all(path) {
        Ok(()) => Removal::Removed,
        Err(_) => Removal::Failed,
    }
}

fn source_name(source: &Path) -> Result<String, PlacementError> {
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned();
    if valid_name(&name) {
        Ok(name)
    } else {
        Err(PlacementError::InvalidName(name))
    }
}

/// One to 64 lowercase ASCII letters, digits or hyphens, starting with a letter or digit.
fn valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() <= 64
        && bytes
            .first()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn canonical_path(home: &Path, name: &str) -> PathBuf {
    home.join(".agents").join("skills").join(name)
}

fn agent_folder(home: &Path, agent: PlacementAgent, name: &str) -> Option<PathBuf> {
    agent.home_folder().map(|segments| {
        segments
            .iter()
            .fold(home.to_path_buf(), |dir, part| dir.join(part))
            .join(name)
    })
}

/// The folders a placement of any skill can write into: the canonical root and each agent's own folder.
fn skill_folders(home: &Path) -> Vec<PathBuf> {
    let mut folders = vec![home.join(".agents").join("skills")];
    for agent in PlacementAgent::ALL {
        if let Some(segments) = agent.home_folder() {
            folders.push(
                segments
                    .iter()
                    .fold(home.to_path_buf(), |dir, part| dir.join(part)),
            );
        }
    }
    folders
}
