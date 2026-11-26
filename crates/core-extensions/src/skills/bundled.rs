//! The skills MassOrca ships (SKL-04). A bundled skill's files are embedded in the binary from `packages/skills/`, and it is
//! placed by the placement path of a home skill (decision 0150): the source is a folder this host writes from those bytes,
//! never a path a caller names. The record keeps the placed digest, so a build with other text sees the copies as outdated,
//! and a refresh brings them up to date.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::placement::{managed_placements, place_skill, unplace_skill};
use super::placement_agents::PlacementAgent;
use super::placement_error::PlacementError;
use super::placement_model::{PlacementReport, PlacementStore};
use super::placement_tree::{FolderDigest, is_plain_folder};

/// One skill this build ships: its folder name and its files, by path below the folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BundledSkill {
    pub name: &'static str,
    files: &'static [(&'static str, &'static [u8])],
}

static BUNDLED: [BundledSkill; 1] = [BundledSkill {
    name: "massorca",
    files: &[(
        "SKILL.md",
        include_bytes!("../../../../packages/skills/massorca/SKILL.md"),
    )],
}];

/// Every skill this build ships.
pub fn bundled_skills() -> &'static [BundledSkill] {
    &BUNDLED
}

/// The bundled skill with this name.
pub fn find_bundled(name: &str) -> Option<&'static BundledSkill> {
    BUNDLED.iter().find(|skill| skill.name == name)
}

impl BundledSkill {
    /// The skill's `SKILL.md`, as this build embeds it.
    pub fn text(&self) -> &'static str {
        self.files
            .iter()
            .find(|(path, _)| *path == "SKILL.md")
            .and_then(|(_, bytes)| std::str::from_utf8(bytes).ok())
            .unwrap_or_default()
    }

    /// The `description` of the skill's front matter.
    pub fn description(&self) -> &'static str {
        let mut lines = self.text().lines();
        if lines.next() != Some("---") {
            return "";
        }
        lines
            .take_while(|line| *line != "---")
            .find_map(|line| line.strip_prefix("description: "))
            .unwrap_or_default()
    }

    /// The digest of the skill folder as placed: the value the record keeps for it. Folders hash in the order the placed copy
    /// is digested, so this equals the digest of a folder written from the same files.
    pub fn digest(&self) -> String {
        let mut files: Vec<(PathBuf, &[u8])> = self
            .files
            .iter()
            .map(|(path, bytes)| (PathBuf::from(path), *bytes))
            .collect();
        files.sort_by(|left, right| left.0.cmp(&right.0));
        let dirs: BTreeSet<PathBuf> = files
            .iter()
            .flat_map(|(path, _)| path.ancestors().skip(1))
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .collect();
        let mut digest = FolderDigest::new();
        for dir in &dirs {
            digest.dir(dir);
        }
        for (path, bytes) in &files {
            digest.file(path, bytes);
        }
        digest.finish()
    }

    /// Writes the skill's files into `staging/<name>`, replacing what an earlier write left there, and returns that folder.
    /// The folder is only ever written from the embedded bytes.
    fn write_source(&self, staging: &Path) -> Result<PathBuf, PlacementError> {
        let folder = staging.join(self.name);
        if fs::symlink_metadata(&folder).is_ok() {
            if !is_plain_folder(&folder) {
                return Err(PlacementError::Unsupported {
                    path: folder,
                    reason: "it is not a plain folder",
                });
            }
            fs::remove_dir_all(&folder).map_err(|source| PlacementError::Io {
                message: "cannot clear the bundled skill",
                path: folder.clone(),
                source,
            })?;
        }
        for (path, bytes) in self.files {
            let target = folder.join(path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|source| PlacementError::Io {
                    message: "cannot create the bundled skill",
                    path: parent.to_path_buf(),
                    source,
                })?;
            }
            fs::write(&target, bytes).map_err(|source| PlacementError::Io {
                message: "cannot write the bundled skill",
                path: target,
                source,
            })?;
        }
        Ok(folder)
    }
}

/// Where a bundled skill stands on this host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundledState {
    /// MassOrca has not placed it.
    NotPlaced,
    /// The placed copies hold this build's text.
    Current,
    /// The placed copies hold another build's text; a refresh brings them up to date.
    Outdated,
}

/// A bundled skill's state, and the agents it is placed for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundledStatus {
    pub skill: &'static BundledSkill,
    pub state: BundledState,
    pub agents: Vec<PlacementAgent>,
}

/// The state of one bundled skill, from the placement record.
pub fn bundled_status(
    store: &PlacementStore,
    skill: &'static BundledSkill,
) -> Result<BundledStatus, PlacementError> {
    let placed = managed_placements(store)?
        .into_iter()
        .find(|managed| managed.name == skill.name);
    let (state, agents) = match placed {
        None => (BundledState::NotPlaced, Vec::new()),
        Some(managed) => {
            let state = if managed.digest == skill.digest() {
                BundledState::Current
            } else {
                BundledState::Outdated
            };
            (state, managed.agents.iter().map(|row| row.agent).collect())
        }
    };
    Ok(BundledStatus {
        skill,
        state,
        agents,
    })
}

/// The state of every bundled skill, in registry order.
pub fn bundled_statuses(store: &PlacementStore) -> Result<Vec<BundledStatus>, PlacementError> {
    bundled_skills()
        .iter()
        .map(|skill| bundled_status(store, skill))
        .collect()
}

/// Places a bundled skill for `agents`, from a folder written under `staging`.
pub fn install_bundled(
    home: &Path,
    store: &PlacementStore,
    staging: &Path,
    skill: &BundledSkill,
    agents: &[PlacementAgent],
) -> Result<PlacementReport, PlacementError> {
    let source = skill.write_source(staging)?;
    place_skill(home, store, &source, agents)
}

/// Brings a placed bundled skill to this build's text. An outdated skill is removed and placed again for the agents it had;
/// when a copy the person changed keeps it, that removal is returned and nothing new is placed. A current skill is placed
/// again for its agents, which refills a folder that went missing and changes nothing else.
pub fn refresh_bundled(
    home: &Path,
    store: &PlacementStore,
    staging: &Path,
    skill: &'static BundledSkill,
) -> Result<PlacementReport, PlacementError> {
    let status = bundled_status(store, skill)?;
    if status.state == BundledState::NotPlaced {
        return Err(PlacementError::NotPlaced(skill.name.to_owned()));
    }
    if status.agents.is_empty() {
        return Err(PlacementError::NoAgents);
    }
    if status.state == BundledState::Outdated {
        let removal = unplace_skill(home, store, skill.name, &[])?;
        if bundled_status(store, skill)?.state != BundledState::NotPlaced {
            return Ok(removal);
        }
    }
    install_bundled(home, store, staging, skill, &status.agents)
}
