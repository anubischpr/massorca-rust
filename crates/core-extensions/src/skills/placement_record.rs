//! The record of the skills MassOrca placed: one JSON file in the profile, written atomically. A missing file is an empty
//! record; a file that does not parse is refused rather than guessed at, so removal never acts on a wrong record.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::placement_agents::PlacementAgent;
use super::placement_error::PlacementError;
use super::placement_model::{ManagedAgent, ManagedSkill, PlacementStore, Topology};
use crate::lockfile::write_atomic;

#[derive(Debug, Default, Serialize, Deserialize)]
struct RecordFile {
    #[serde(default)]
    skills: Vec<RecordSkill>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecordSkill {
    name: String,
    canonical: PathBuf,
    digest: String,
    #[serde(default)]
    agents: Vec<RecordAgent>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecordAgent {
    agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path: Option<PathBuf>,
    topology: Topology,
}

/// Every skill in the record.
pub(super) fn load(store: &PlacementStore) -> Result<Vec<ManagedSkill>, PlacementError> {
    let text = match fs::read_to_string(store.path()) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(PlacementError::Io {
                message: "cannot read the placement record",
                path: store.path().to_path_buf(),
                source,
            });
        }
    };
    let file: RecordFile = serde_json::from_str(&text)
        .map_err(|error| PlacementError::InvalidRecord(error.to_string()))?;
    file.skills.into_iter().map(into_managed).collect()
}

/// Replaces the record with `skills`, atomically.
pub(super) fn save(store: &PlacementStore, skills: &[ManagedSkill]) -> Result<(), PlacementError> {
    let file = RecordFile {
        skills: skills.iter().map(from_managed).collect(),
    };
    let bytes = serde_json::to_vec_pretty(&file)
        .map_err(|error| PlacementError::InvalidRecord(error.to_string()))?;
    write_atomic(store.path(), &bytes).map_err(PlacementError::Store)
}

fn into_managed(skill: RecordSkill) -> Result<ManagedSkill, PlacementError> {
    let agents = skill
        .agents
        .into_iter()
        .map(|row| {
            let agent = PlacementAgent::from_id(&row.agent).ok_or_else(|| {
                PlacementError::InvalidRecord(format!("an unknown agent {:?}", row.agent))
            })?;
            Ok(ManagedAgent {
                agent,
                path: row.path,
                topology: row.topology,
            })
        })
        .collect::<Result<Vec<_>, PlacementError>>()?;
    Ok(ManagedSkill {
        name: skill.name,
        canonical: skill.canonical,
        digest: skill.digest,
        agents,
    })
}

fn from_managed(skill: &ManagedSkill) -> RecordSkill {
    RecordSkill {
        name: skill.name.clone(),
        canonical: skill.canonical.clone(),
        digest: skill.digest.clone(),
        agents: skill
            .agents
            .iter()
            .map(|row| RecordAgent {
                agent: row.agent.id().to_owned(),
                path: row.path.clone(),
                topology: row.topology,
            })
            .collect(),
    }
}
