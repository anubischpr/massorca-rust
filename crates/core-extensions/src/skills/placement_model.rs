//! The types a skill placement reports and records: the store, how an agent folder reaches the canonical copy, what happened
//! for each agent, and the skills MassOrca placed.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::placement_agents::PlacementAgent;

/// The record of the skills MassOrca placed, in one JSON file of the profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementStore {
    path: PathBuf,
}

impl PlacementStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// How an agent's folder reaches the canonical copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Topology {
    /// The agent reads the canonical copy itself.
    Canonical,
    /// The agent's folder is a link to the canonical copy.
    Alias,
    /// The agent's folder is a verified copy, made when a link was refused or unavailable.
    Copy,
}

/// Whether placement tries links first. `Unavailable` makes every folder a copy, as on a filesystem that refuses links.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkMode {
    Prefer,
    Unavailable,
}

/// Why a folder was left alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipCause {
    /// The agent's folder holds something MassOrca did not place.
    Existing,
    /// The placement MassOrca made was changed since, so it is kept.
    Modified,
    /// The copy failed; the canonical copy and the other agents are unaffected, and placing again retries it.
    CopyFailed,
}

/// What happened for one agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The agent reads the canonical copy, so it needs no folder of its own.
    Canonical,
    Placed(Topology),
    AlreadyPlaced(Topology),
    Removed,
    Skipped(SkipCause),
}

/// One agent's row of a placement or removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentOutcome {
    pub agent: PlacementAgent,
    /// The agent's own folder; `None` when it reads the canonical copy.
    pub path: Option<PathBuf>,
    pub outcome: Outcome,
}

/// The result of a placement or a removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementReport {
    pub name: String,
    pub canonical: PathBuf,
    pub agents: Vec<AgentOutcome>,
}

/// What a placement would do for one agent, without writing anything. A new folder is planned as an alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    Canonical,
    Create,
    AlreadyPlaced(Topology),
    Skipped(SkipCause),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedAgent {
    pub agent: PlacementAgent,
    pub path: Option<PathBuf>,
    pub plan: Plan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementPreview {
    pub name: String,
    pub canonical: PathBuf,
    pub agents: Vec<PlannedAgent>,
}

/// One skill MassOrca placed, with its canonical copy and each agent's folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSkill {
    pub name: String,
    pub canonical: PathBuf,
    pub digest: String,
    pub agents: Vec<ManagedAgent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedAgent {
    pub agent: PlacementAgent,
    pub path: Option<PathBuf>,
    pub topology: Topology,
}
