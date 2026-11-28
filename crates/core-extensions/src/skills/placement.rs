//! Placing a skill for agent tools (SKL-02, as pv96 places it): one canonical copy at `.agents/skills/<name>`, then in each
//! chosen agent's folder an alias to that copy (a relative symlink on Unix, a junction on Windows), or a verified copy when
//! an alias is refused. Removal checks each placement by how it was made: an alias by its target, a copy by its digest. A
//! folder the user already has is never overwritten. The root is the home folder for a global placement, or a workspace's
//! root for a workspace placement (decision 0150); the same rules apply to both.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::placement_agents::PlacementAgent;
use super::placement_error::PlacementError;
use super::placement_link::{create_link, link_points_to, remove_link};
use super::placement_model::{
    AgentOutcome, LinkMode, ManagedAgent, ManagedSkill, Outcome, PlacementPreview, PlacementReport,
    PlacementScope, PlacementStore, Plan, PlannedAgent, SkipCause, Topology,
};
use super::placement_record::{load, save};
use super::placement_tree::{is_link, is_plain_folder, place_copy, sweep_staging, tree_digest};

/// What placing `source` in the global folders would do, and the refusals that would stop it. Writes nothing.
pub fn preview_placement(
    home: &Path,
    store: &PlacementStore,
    source: &Path,
    agents: &[PlacementAgent],
) -> Result<PlacementPreview, PlacementError> {
    preview_placement_in(home, &PlacementScope::Global, store, source, agents)
}

/// What placing `source` under `root` for `scope` would do. Writes nothing.
pub fn preview_placement_in(
    root: &Path,
    scope: &PlacementScope,
    store: &PlacementStore,
    source: &Path,
    agents: &[PlacementAgent],
) -> Result<PlacementPreview, PlacementError> {
    let prepared = prepare(root, scope, store, source, agents)?;
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

/// Places `source` in the global folders, with links where the filesystem allows them, and copies elsewhere.
pub fn place_skill(
    home: &Path,
    store: &PlacementStore,
    source: &Path,
    agents: &[PlacementAgent],
) -> Result<PlacementReport, PlacementError> {
    place_skill_in(home, &PlacementScope::Global, store, source, agents)
}

/// Places `source` under `root` for `scope`, with links where the filesystem allows them, and copies elsewhere.
pub fn place_skill_in(
    root: &Path,
    scope: &PlacementScope,
    store: &PlacementStore,
    source: &Path,
    agents: &[PlacementAgent],
) -> Result<PlacementReport, PlacementError> {
    place_skill_with_in(root, scope, store, source, agents, LinkMode::Prefer)
}

/// Places `source` in the global folders as the canonical copy and in each chosen agent's folder. A folder that cannot take
/// a link is copied and verified; one that cannot be placed is skipped and reported, and the canonical copy and the rest
/// stand.
pub fn place_skill_with(
    home: &Path,
    store: &PlacementStore,
    source: &Path,
    agents: &[PlacementAgent],
    links: LinkMode,
) -> Result<PlacementReport, PlacementError> {
    place_skill_with_in(home, &PlacementScope::Global, store, source, agents, links)
}

/// Places `source` under `root` for `scope`; see [`place_skill_with`].
pub fn place_skill_with_in(
    root: &Path,
    scope: &PlacementScope,
    store: &PlacementStore,
    source: &Path,
    agents: &[PlacementAgent],
    links: LinkMode,
) -> Result<PlacementReport, PlacementError> {
    let prepared = prepare(root, scope, store, source, agents)?;
    for folder in skill_folders(root) {
        sweep_staging(&folder, &prepared.name);
    }
    if !prepared.canonical_managed {
        place_verified_copy(source, &prepared.canonical, &prepared.digest)?;
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
        .position(|skill| skill.name == prepared.name && skill.scope == *scope)
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
        scope: scope.clone(),
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

/// Removes the global MassOrca placements of `name` for `agents`; see [`unplace_skill_in`].
pub fn unplace_skill(
    home: &Path,
    store: &PlacementStore,
    name: &str,
    agents: &[PlacementAgent],
) -> Result<PlacementReport, PlacementError> {
    unplace_skill_in(home, &PlacementScope::Global, store, name, agents)
}

/// Removes the MassOrca placements of `name` for `scope` under `root`, for `agents`; an empty list means every agent MassOrca
/// placed for. A link is removed as a link, never through to the canonical copy; a copy the user changed is kept and
/// reported. The canonical copy goes once no placement in the same scope depends on it.
pub fn unplace_skill_in(
    root: &Path,
    scope: &PlacementScope,
    store: &PlacementStore,
    name: &str,
    agents: &[PlacementAgent],
) -> Result<PlacementReport, PlacementError> {
    let mut skills = load(store)?;
    let index = skills
        .iter()
        .position(|skill| skill.name == name && skill.scope == *scope)
        .ok_or_else(|| PlacementError::NotPlaced(name.to_owned()))?;
    let mut skill = skills.remove(index);
    let canonical = canonical_path(root, name);
    for folder in skill_folders(root) {
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
        let expected = agent_folder(root, row.agent, name);
        // Why: the record never names what is deleted; only the folder this agent reads can go.
        let outcome = if row.path != expected {
            Outcome::Skipped(SkipCause::Modified)
        } else {
            match expected.as_deref() {
                // A canonical row is settled once the canonical copy's fate is known.
                None => Outcome::Removed,
                Some(path) => removal_outcome(&remove_placement(
                    path,
                    row.topology,
                    &skill.digest,
                    &canonical,
                )),
            }
        };
        if matches!(outcome, Outcome::Skipped(_)) {
            kept.push(row.clone());
        }
        rows.push(AgentOutcome {
            agent: row.agent,
            path: row.path,
            outcome,
        });
    }

    // Why: the canonical copy goes only when nothing still reaches it, so no alias is left dangling.
    let canonical_removal = kept
        .is_empty()
        .then(|| remove_copy(&canonical, &skill.digest));
    for row in &mut rows {
        if row.path.is_none() && row.outcome == Outcome::Removed {
            row.outcome = match &canonical_removal {
                None => Outcome::Canonical,
                Some(removal) => removal_outcome(removal),
            };
            if matches!(row.outcome, Outcome::Skipped(_)) {
                kept.push(ManagedAgent {
                    agent: row.agent,
                    path: None,
                    topology: Topology::Canonical,
                });
            }
        }
    }
    let canonical_gone = matches!(canonical_removal, Some(Removal::Gone | Removal::Removed));
    if !canonical_gone {
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

/// Adds `add` agents to a placed skill and drops `drop` agents, in one scope under `root`. The canonical copy is what an
/// added folder is placed from, so no discovered skill is needed. Adding creates only the new agents' folders; dropping
/// removes only their copies, and the canonical copy stays while another agent still reaches it.
pub fn switch_agents_in(
    root: &Path,
    scope: &PlacementScope,
    store: &PlacementStore,
    name: &str,
    add: &[PlacementAgent],
    drop: &[PlacementAgent],
) -> Result<PlacementReport, PlacementError> {
    let placed = load(store)?
        .iter()
        .any(|skill| skill.name == name && skill.scope == *scope);
    if !placed {
        return Err(PlacementError::NotPlaced(name.to_owned()));
    }
    let canonical = canonical_path(root, name);
    let mut rows = Vec::new();
    if !add.is_empty() {
        let added = place_skill_with_in(root, scope, store, &canonical, add, LinkMode::Prefer)?;
        rows.extend(added.agents);
    }
    if !drop.is_empty() {
        let dropped = unplace_skill_in(root, scope, store, name, drop)?;
        rows.extend(dropped.agents);
    }
    Ok(PlacementReport {
        name: name.to_owned(),
        canonical,
        agents: rows,
    })
}

/// The skills MassOrca placed, as recorded, in every scope.
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
    root: &Path,
    scope: &PlacementScope,
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
    let recorded = load(store)?
        .into_iter()
        .find(|skill| skill.name == name && skill.scope == *scope);
    if recorded
        .as_ref()
        .is_some_and(|skill| skill.digest != digest)
    {
        return Err(PlacementError::ContentChanged(name));
    }

    let canonical = canonical_path(root, &name);
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
        .map(|agent| match agent_folder(root, agent, &name) {
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
    match place_verified_copy(source, path, digest) {
        Ok(()) => Outcome::Placed(Topology::Copy),
        Err(_) => Outcome::Skipped(SkipCause::CopyFailed),
    }
}

/// Copies `source` to `target` and keeps the copy only when it matches `digest`.
fn place_verified_copy(source: &Path, target: &Path, digest: &str) -> Result<(), PlacementError> {
    place_copy(source, target)?;
    if tree_digest(target).ok().as_deref() == Some(digest) {
        return Ok(());
    }
    let _ = fs::remove_dir_all(target);
    Err(PlacementError::Io {
        message: "the copy does not match the skill",
        path: target.to_path_buf(),
        source: io::Error::from(io::ErrorKind::InvalidData),
    })
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

fn removal_outcome(removal: &Removal) -> Outcome {
    match removal {
        Removal::Gone | Removal::Removed => Outcome::Removed,
        Removal::Modified => Outcome::Skipped(SkipCause::Modified),
        Removal::Failed => Outcome::Skipped(SkipCause::CopyFailed),
    }
}

/// Removes one placement by how it was made: an alias only while it still points at the canonical copy, and it is removed
/// as a link; a copy only while its digest still matches. `path` is the agent's own folder, never one the record names.
fn remove_placement(path: &Path, topology: Topology, digest: &str, canonical: &Path) -> Removal {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return Removal::Gone;
    };
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
        Topology::Copy | Topology::Canonical => remove_copy(path, digest),
    }
}

/// Removes a copy when it is still a plain folder with the recorded digest.
fn remove_copy(path: &Path, digest: &str) -> Removal {
    if fs::symlink_metadata(path).is_err() {
        return Removal::Gone;
    }
    if !is_plain_folder(path) || tree_digest(path).ok().as_deref() != Some(digest) {
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
pub(super) fn valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() <= 64
        && bytes
            .first()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn canonical_path(root: &Path, name: &str) -> PathBuf {
    root.join(".agents").join("skills").join(name)
}

fn agent_folder(root: &Path, agent: PlacementAgent, name: &str) -> Option<PathBuf> {
    agent.home_folder().map(|segments| {
        segments
            .iter()
            .fold(root.to_path_buf(), |dir, part| dir.join(part))
            .join(name)
    })
}

/// The folders a placement of any skill can write into under `root`: the canonical root and each agent's own folder.
fn skill_folders(root: &Path) -> Vec<PathBuf> {
    let mut folders = vec![root.join(".agents").join("skills")];
    for agent in PlacementAgent::ALL {
        if let Some(segments) = agent.home_folder() {
            folders.push(
                segments
                    .iter()
                    .fold(root.to_path_buf(), |dir, part| dir.join(part)),
            );
        }
    }
    folders
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_that_does_not_match_the_digest_is_removed_and_refused() {
        let temp = tempfile::TempDir::new().expect("a temporary folder");
        let source = temp.path().join("source");
        fs::create_dir_all(&source).expect("the source");
        fs::write(source.join("SKILL.md"), "body").expect("the file");
        let target = temp.path().join("skills/release-notes");

        let result = place_verified_copy(&source, &target, "not-the-digest");

        assert!(matches!(result, Err(PlacementError::Io { .. })));
        assert!(!target.exists(), "the unverified copy is removed");
        let digest = tree_digest(&source).expect("a digest");
        place_verified_copy(&source, &target, &digest).expect("a matching copy stays");
        assert!(target.join("SKILL.md").is_file());
    }
}
