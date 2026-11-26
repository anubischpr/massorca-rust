//! Skill placement (SKL-02, capability `skills-place/1`): `skills.place` copies a skill that discovery found into the folders
//! of the chosen agent tools, and `skills.unplace` removes only the copies MassOrca placed and left unchanged. The preview
//! and the list of managed skills only read. Writes need host administration. Every enum has an `Unknown` variant for
//! values from a newer peer.

use serde::{Deserialize, Serialize};

/// Params: [`SkillPlaceParams`]. Result: [`SkillPlacementResult`]. Changes nothing.
pub const PREVIEW: &str = "skills.place-preview";
/// Params: [`SkillPlaceParams`]. Result: [`SkillPlacementResult`].
pub const PLACE: &str = "skills.place";
/// Params: [`SkillUnplaceParams`]. Result: [`SkillPlacementResult`].
pub const UNPLACE: &str = "skills.unplace";
/// Params: [`SkillsManagedParams`]. Result: [`SkillsManagedResult`].
pub const MANAGED: &str = "skills.managed";

/// An agent tool a skill can be placed for. The ids match the registry of pv96.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlacementAgent {
    Codex,
    Claude,
    Cursor,
    Gemini,
    Droid,
    Continue,
    Trae,
    Grok,
    Aug,
    Muse,
    Zcode,
    /// A value from a newer peer that this build does not know; refused, never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Params of `skills.place-preview` and `skills.place`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillPlaceParams {
    /// The `id` of a home skill that `skills.discover` lists.
    pub skill_id: String,
    pub agents: Vec<PlacementAgent>,
}

/// Params of `skills.unplace`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillUnplaceParams {
    /// The skill's name, as the managed list shows it.
    pub name: String,
    /// Empty removes every copy MassOrca placed for the skill.
    #[serde(default)]
    pub agents: Vec<PlacementAgent>,
}

/// Params of `skills.managed`: none.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillsManagedParams {}

/// What a preview plans, or what a placement or removal did, for each agent. Paths are left out for a caller without host
/// administration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillPlacementResult {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_path: Option<String>,
    pub agents: Vec<AgentPlacement>,
}

/// One agent's row of a placement, a removal or a preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPlacement {
    pub agent: PlacementAgent,
    pub state: PlacementState,
    /// The agent's own folder; left out for a caller without host administration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The agent reads the canonical copy and has no folder of its own.
    #[serde(default)]
    pub canonical: bool,
    /// How the folder reaches the canonical copy; absent in a preview for a folder not yet made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topology: Option<PlacementTopology>,
}

/// What happened for one agent. `create` appears only in a preview; `skipped_*` names why a folder was left alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlacementState {
    Canonical,
    Create,
    Placed,
    AlreadyPlaced,
    Removed,
    SkippedExisting,
    SkippedModified,
    SkippedCopyFailed,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}

/// Result of `skills.managed`: every skill MassOrca placed on this host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillsManagedResult {
    pub skills: Vec<ManagedSkill>,
}

/// One skill MassOrca placed, with its canonical copy and the agents it was placed for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedSkill {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_path: Option<String>,
    pub agents: Vec<ManagedAgent>,
}

/// One agent a managed skill is placed for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedAgent {
    pub agent: PlacementAgent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default)]
    pub canonical: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topology: Option<PlacementTopology>,
}

/// How an agent's folder reaches the canonical copy: the agent reads it itself, the folder is an alias to it, or the
/// folder is a verified copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlacementTopology {
    Canonical,
    Alias,
    Copy,
    /// A value from a newer peer that this build does not know; never sent by this build.
    #[serde(other)]
    Unknown,
}
