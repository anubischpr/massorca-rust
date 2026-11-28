//! Skill placement (SKL-02, capability `skills-place/1`; decision 0150): `skills.place` copies a skill that discovery found
//! into the folders of the chosen agent tools, and `skills.unplace` removes only the copies MassOrca placed and left
//! unchanged. The preview and the list of managed skills only read. Writes need host administration. Every enum has an
//! `Unknown` variant for values from a newer peer.
//!
//! Capability `skills-place/2` adds the workspace scope (`scope` and `workspace_id` on the params) and `skills.agents`, which
//! adds and drops agents of a placed skill. A client sends `scope: workspace` only to a service that offers `/2`: an older
//! service would ignore the field and place the copies globally.

use serde::{Deserialize, Serialize};

/// Params: [`SkillPlaceParams`]. Result: [`SkillPlacementResult`]. Changes nothing.
pub const PREVIEW: &str = "skills.place-preview";
/// Params: [`SkillPlaceParams`]. Result: [`SkillPlacementResult`].
pub const PLACE: &str = "skills.place";
/// Params: [`SkillUnplaceParams`]. Result: [`SkillPlacementResult`].
pub const UNPLACE: &str = "skills.unplace";
/// Params: [`SkillsManagedParams`]. Result: [`SkillsManagedResult`].
pub const MANAGED: &str = "skills.managed";
/// Params: [`SkillAgentsParams`]. Result: [`SkillPlacementResult`]. Capability `skills-place/2`: adds and drops agents of a
/// placed skill in one scope, without re-placing the others.
pub const AGENTS: &str = "skills.agents";

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

/// Where a placement is made: the home folder (`global`, the default) or one workspace's folders (`workspace`). A value
/// from a newer peer decodes as `Unknown` and is refused, never placed globally.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlacementScope {
    #[default]
    Global,
    Workspace,
    /// A value from a newer peer that this build does not know; refused, never sent by this build.
    #[serde(other)]
    Unknown,
}

/// The scope fields every placement call takes, flattened into its params. Absent fields mean a global placement.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementTarget {
    #[serde(default)]
    pub scope: PlacementScope,
    /// The MassOrca workspace id a `workspace` scope names; refused with `global`, and never a free path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
}

/// Params of `skills.place-preview` and `skills.place`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillPlaceParams {
    /// The `id` of a home skill that `skills.discover` lists.
    pub skill_id: String,
    pub agents: Vec<PlacementAgent>,
    #[serde(flatten)]
    pub target: PlacementTarget,
}

/// Params of `skills.unplace`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillUnplaceParams {
    /// The skill's name, as the managed list shows it.
    pub name: String,
    /// Empty removes every copy MassOrca placed for the skill in the scope.
    #[serde(default)]
    pub agents: Vec<PlacementAgent>,
    #[serde(flatten)]
    pub target: PlacementTarget,
}

/// Params of `skills.agents`: the agents to add to and to drop from a placed skill in one scope. Adding creates only the new
/// agents' folders; dropping removes only their copies, and the canonical copy stays while another agent still reaches it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillAgentsParams {
    /// The placed skill's name, as `skills.managed` shows it.
    pub name: String,
    #[serde(default)]
    pub add: Vec<PlacementAgent>,
    #[serde(default)]
    pub drop: Vec<PlacementAgent>,
    #[serde(flatten)]
    pub target: PlacementTarget,
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

/// Result of `skills.managed`: every skill MassOrca placed on this host, in every scope.
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
    /// The workspace the placement is for; absent for a global placement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
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
